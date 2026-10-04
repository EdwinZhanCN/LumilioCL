use super::error::ServiceError;
use super::types::now;
use super::{CREDENTIAL_PROBE, LauncherService, TOKEN_MARGIN_SECONDS};
use crate::account::AuthSession;
use crate::activity::CancellationToken;
use crate::credentials::StoredLogin;
use crate::microsoft::{AuthError, DeviceCode, MicrosoftClient, Secret};
use crate::settings::{AccountKind, SettingsError};
use crate::transfer::Transport;

impl<T: Transport + Clone> LauncherService<T> {
    /// Adds an offline account (optionally with its own profile id). The first
    /// account becomes the selected one.
    pub async fn add_account(&self, name: &str, uuid: Option<&str>) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.add_offline_account(name, uuid)?)
    }

    /// Chooses the account later launches use (by key); running games keep theirs.
    pub async fn select_account(&self, key: &str) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.select_account(key)?)
    }

    /// Forgets an account identity (by key); instances and worlds are
    /// untouched. A Microsoft account's stored sign-in goes with it.
    pub async fn remove_account(&self, key: &str) -> Result<(), ServiceError> {
        let removed = self
            .settings
            .lock()
            .await
            .get()
            .accounts
            .iter()
            .find(|entry| entry.key() == key)
            .map(|entry| entry.kind);
        let signed_in = self
            .settings
            .lock()
            .await
            .get()
            .accounts
            .iter()
            .find(|entry| entry.key() == key)
            .cloned();
        self.settings.lock().await.remove_account(key)?;
        if let Some(entry) = signed_in.filter(|entry| entry.kind == AccountKind::ThirdParty) {
            self.sign_out_third_party(key, &entry).await;
        }
        if matches!(
            removed,
            Some(AccountKind::Microsoft | AccountKind::ThirdParty)
        ) {
            // The account is gone either way; a secret that cannot be deleted
            // is told, not hidden.
            self.credentials.delete(key)?;
        }
        Ok(())
    }

    /// The identity a launch presents: the selected account's real values,
    /// refreshing a Microsoft sign-in that is about to expire.
    pub(super) async fn session_for(
        &self,
        settings: &crate::settings::LauncherSettings,
    ) -> Result<AuthSession, ServiceError> {
        let entry = settings
            .selected_account
            .as_deref()
            .and_then(|key| settings.accounts.iter().find(|entry| entry.key() == key))
            .ok_or(ServiceError::NoAccount)?;
        match entry.kind {
            AccountKind::Offline => {
                let profile = entry.profile().map_err(|_| ServiceError::NoAccount)?;
                match &entry.skin {
                    None => Ok(profile.session()),
                    Some(choice) => self.offline_skin_session(&profile, choice).await,
                }
            }
            AccountKind::Microsoft => self.microsoft_session(entry, false).await,
            AccountKind::ThirdParty => self.third_party_session(entry, false).await,
        }
    }

    /// A Microsoft account's session: the cached Minecraft token while it has
    /// five minutes left, else a refreshed one. `force` refreshes regardless.
    pub(super) async fn microsoft_session(
        &self,
        entry: &crate::settings::AccountEntry,
        force: bool,
    ) -> Result<AuthSession, ServiceError> {
        let _one_at_a_time = self.auth_lock.lock().await;
        let key = entry.key();
        let profile_id = entry
            .profile_id()
            .map_err(|_| ServiceError::SignInRequired(entry.name.clone()))?;
        let stored = StoredLogin::load(self.credentials.as_ref(), &key)?;
        let Some(stored) = stored else {
            self.flag_sign_in(&key).await;
            return Err(ServiceError::SignInRequired(entry.name.clone()));
        };
        if !force && stored.expires_at > now() + TOKEN_MARGIN_SECONDS {
            return Ok(AuthSession::online(
                &entry.name,
                profile_id,
                &stored.access_token,
            ));
        }
        let client = MicrosoftClient::new(&self.transport);
        let oauth = match client.refresh(&Secret::new(stored.refresh_token)).await {
            Ok(oauth) => oauth,
            Err(AuthError::SignInRequired) => {
                self.flag_sign_in(&key).await;
                return Err(ServiceError::SignInRequired(entry.name.clone()));
            }
            Err(error) => return Err(error.into()),
        };
        // The refresh token has rotated: keep the new one before anything else.
        StoredLogin {
            refresh_token: oauth.refresh_token.expose().to_owned(),
            access_token: stored.access_token,
            expires_at: stored.expires_at,
        }
        .save(self.credentials.as_ref(), &key)?;
        let login = client.minecraft_login(&oauth).await?;
        if login.profile_id != profile_id {
            // Another Minecraft profile than the one this account was added
            // for: do not play as someone else.
            self.flag_sign_in(&key).await;
            return Err(ServiceError::SignInRequired(entry.name.clone()));
        }
        StoredLogin {
            refresh_token: oauth.refresh_token.expose().to_owned(),
            access_token: login.access_token.expose().to_owned(),
            expires_at: login.expires_at,
        }
        .save(self.credentials.as_ref(), &key)?;
        // A renamed profile shows its new name.
        if login.profile_name != entry.name {
            let _ = self
                .settings
                .lock()
                .await
                .sign_in_microsoft(login.profile_id, &login.profile_name);
        }
        Ok(AuthSession::online(
            &login.profile_name,
            login.profile_id,
            login.access_token.expose(),
        ))
    }

    pub(super) async fn flag_sign_in(&self, key: &str) {
        // Best effort: the launch is already failing with a clearer error.
        let _ = self.settings.lock().await.set_needs_sign_in(key, true);
    }

    /// Signs a Microsoft account in with the device code flow. `on_code` is
    /// given the code to show the person; this then waits (polling at the
    /// service's pace) until they finish in the browser, declines, the code
    /// runs out, or `cancel` fires. On success the account is added (or
    /// updated, when its profile is already known) and selected if first, its
    /// secrets go to the credential store, and its key and name are returned.
    pub async fn microsoft_sign_in(
        &self,
        on_code: impl FnOnce(&DeviceCode) + Send,
        cancel: CancellationToken,
    ) -> Result<(String, String), ServiceError> {
        // Fail before showing a code that could never be kept.
        self.credentials.get(CREDENTIAL_PROBE)?;
        let client = MicrosoftClient::new(&self.transport);
        let code = client.request_device_code().await?;
        on_code(&code);
        let oauth = client.wait_for_sign_in(&code, &cancel).await?;
        let login = tokio::select! {
            () = cancel.cancelled() => return Err(AuthError::Cancelled.into()),
            login = client.minecraft_login(&oauth) => login?,
        };
        let key = format!("msa:{}", login.profile_id.compact());
        StoredLogin {
            refresh_token: oauth.refresh_token.expose().to_owned(),
            access_token: login.access_token.expose().to_owned(),
            expires_at: login.expires_at,
        }
        .save(self.credentials.as_ref(), &key)?;
        let recorded = self
            .settings
            .lock()
            .await
            .sign_in_microsoft(login.profile_id, &login.profile_name);
        if let Err(error) = recorded {
            // No account entry means no use for the secret.
            let _ = self.credentials.delete(&key);
            return Err(error.into());
        }
        Ok((key, login.profile_name))
    }

    /// Refreshes a Microsoft account's sign-in now (the account's ⋯ menu), so
    /// a problem shows before the next launch rather than during it.
    pub async fn refresh_account(&self, key: &str) -> Result<(), ServiceError> {
        let entry = self
            .settings
            .lock()
            .await
            .get()
            .accounts
            .iter()
            .find(|entry| entry.key() == key)
            .cloned()
            .ok_or_else(|| ServiceError::Settings(SettingsError::UnknownAccount(key.to_owned())))?;
        match entry.kind {
            AccountKind::Offline => return Ok(()),
            AccountKind::Microsoft => self.microsoft_session(&entry, true).await?,
            AccountKind::ThirdParty => self.third_party_session(&entry, true).await?,
        };
        // A good refresh clears an earlier "must sign in again".
        self.settings.lock().await.set_needs_sign_in(key, false)?;
        Ok(())
    }
}
