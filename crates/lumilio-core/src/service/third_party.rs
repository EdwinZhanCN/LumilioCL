//! Accounts on authlib-injector servers (LittleSkin and others): adding
//! servers, signing in, keeping the session alive, and the identity a launch
//! presents. The rules follow HMCL (`YggdrasilAccount`, `AuthlibInjectorAccount`;
//! ADR 0011); where secrets live is ADR 0020.

use std::sync::atomic::Ordering;

use super::error::ServiceError;
use super::{CREDENTIAL_PROBE, LauncherService};
use crate::account::{AuthSession, Injection, ProfileId};
use crate::credentials::StoredYggdrasil;
use crate::injector;
use crate::microsoft::Secret;
use crate::settings::{AccountEntry, AccountKind, AuthServerEntry, SettingsError};
use crate::transfer::Transport;
use crate::yggdrasil::{
    AuthServer, Profile, Session, YggdrasilClient, YggdrasilError, fetch_metadata, little_skin,
    locate_server,
};

/// A sign-in that is waiting for the person to choose a character. The
/// session (and its token) stays in memory until it is chosen or dropped.
pub(super) struct PendingSignIn {
    server: String,
    login: String,
    session: Session,
}

/// Where a third-party sign-in stands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ThirdPartySignIn {
    /// Signed in: the account's key and its character's name.
    Done { key: String, name: String },
    /// The account has several characters: choose one with the `pending` id.
    Choose {
        pending: u64,
        characters: Vec<Profile>,
    },
}

/// 32 random hex digits, as HMCL's client tokens are.
fn random_client_token(fixed: Option<&String>) -> Result<String, ServiceError> {
    if let Some(token) = fixed {
        return Ok(token.clone());
    }
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| {
        ServiceError::Yggdrasil(YggdrasilError::Network(format!(
            "no source of randomness: {error}"
        )))
    })?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

impl<T: Transport + Clone> LauncherService<T> {
    /// The authentication servers to offer: LittleSkin, then the ones added.
    pub async fn auth_servers(&self) -> Vec<AuthServer> {
        let settings = self.settings.lock().await.get().clone();
        std::iter::once(little_skin())
            .chain(settings.auth_servers.into_iter().map(|entry| AuthServer {
                url: entry.url,
                name: entry.name,
                non_email_login: entry.non_email_login,
                links: std::collections::BTreeMap::new(),
            }))
            .collect()
    }

    /// Finds the server an address means, without remembering it.
    pub async fn locate_auth_server(&self, address: &str) -> Result<AuthServer, ServiceError> {
        Ok(locate_server(&self.transport, address).await?)
    }

    /// Remembers a server found with [`Self::locate_auth_server`].
    pub async fn add_auth_server(&self, server: &AuthServer) -> Result<(), ServiceError> {
        Ok(self
            .settings
            .lock()
            .await
            .add_auth_server(AuthServerEntry {
                url: server.url.clone(),
                name: server.name.clone(),
                non_email_login: server.non_email_login,
            })?)
    }

    /// Forgets a server and the accounts signed in on it (their secrets go
    /// too). LittleSkin is built in and stays.
    pub async fn remove_auth_server(&self, url: &str) -> Result<(), ServiceError> {
        let removed = self.settings.lock().await.remove_auth_server(url)?;
        // The accounts are gone either way; a secret that cannot be deleted is told.
        for key in removed {
            self.credentials.delete(&key)?;
        }
        Ok(())
    }

    /// The server's own record for `url`, built in or added.
    async fn auth_server(&self, url: &str) -> Result<AuthServer, ServiceError> {
        self.auth_servers()
            .await
            .into_iter()
            .find(|server| server.url == url)
            .ok_or_else(|| ServiceError::Settings(SettingsError::UnknownServer(url.to_owned())))
    }

    /// Signs in with a name and password. With several characters the answer
    /// asks for a choice, finished with [`Self::third_party_choose`].
    pub async fn third_party_sign_in(
        &self,
        server: &str,
        login: &str,
        password: &str,
    ) -> Result<ThirdPartySignIn, ServiceError> {
        let known = self.auth_server(server).await?;
        // Fail before asking the server for a session that could never be kept.
        self.credentials.get(CREDENTIAL_PROBE)?;
        let client_token = random_client_token(self.client_token_override.as_ref())?;
        let client = YggdrasilClient::new(&self.transport, &known.url);
        let session = client.authenticate(login, password, &client_token).await?;
        if session.selected.is_some() {
            return self.finish_third_party(&known.url, login, session).await;
        }
        match session.available.len() {
            0 => Err(YggdrasilError::NoCharacter.into()),
            1 => {
                let only = session.available[0].clone();
                let chosen = client
                    .refresh(&session.access_token, &client_token, Some(&only))
                    .await?;
                self.finish_third_party(&known.url, login, chosen).await
            }
            _ => {
                let characters = session.available.clone();
                let pending = self.next_pending.fetch_add(1, Ordering::SeqCst);
                self.pending_sign_ins
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(
                        pending,
                        PendingSignIn {
                            server: known.url,
                            login: login.to_owned(),
                            session,
                        },
                    );
                Ok(ThirdPartySignIn::Choose {
                    pending,
                    characters,
                })
            }
        }
    }

    /// Chooses the character for a sign-in that asked. Returns the account's
    /// key and the character's name.
    pub async fn third_party_choose(
        &self,
        pending: u64,
        character: ProfileId,
    ) -> Result<(String, String), ServiceError> {
        let waiting = self
            .pending_sign_ins
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&pending)
            .ok_or(ServiceError::NoPendingSignIn)?;
        let choice = waiting
            .session
            .available
            .iter()
            .find(|profile| profile.id == character)
            .cloned()
            .ok_or(ServiceError::NoPendingSignIn)?;
        let client = YggdrasilClient::new(&self.transport, &waiting.server);
        let chosen = client
            .refresh(
                &waiting.session.access_token,
                &waiting.session.client_token,
                Some(&choice),
            )
            .await?;
        match self
            .finish_third_party(&waiting.server, &waiting.login, chosen)
            .await?
        {
            ThirdPartySignIn::Done { key, name } => Ok((key, name)),
            ThirdPartySignIn::Choose { .. } => Err(ServiceError::NoPendingSignIn),
        }
    }

    /// Drops a sign-in that was waiting for a choice (the person walked away).
    pub fn third_party_abandon(&self, pending: u64) {
        self.pending_sign_ins
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&pending);
    }

    async fn finish_third_party(
        &self,
        server: &str,
        login: &str,
        session: Session,
    ) -> Result<ThirdPartySignIn, ServiceError> {
        let selected = session
            .selected
            .clone()
            .ok_or(YggdrasilError::NoCharacter)?;
        if selected.name.trim().is_empty() {
            return Err(YggdrasilError::Malformed("the character has no name".to_owned()).into());
        }
        let key = format!("ali:{}@{server}", selected.id.compact());
        StoredYggdrasil {
            client_token: session.client_token.clone(),
            access_token: session.access_token.expose().to_owned(),
            user_properties: session.user_properties.clone(),
        }
        .save(self.credentials.as_ref(), &key)?;
        let recorded = self.settings.lock().await.sign_in_third_party(
            server,
            selected.id,
            &selected.name,
            login,
        );
        if let Err(error) = recorded {
            // No account entry means no use for the secret.
            let _ = self.credentials.delete(&key);
            return Err(error.into());
        }
        Ok(ThirdPartySignIn::Done {
            key,
            name: selected.name,
        })
    }

    /// Tells the server to forget a removed account's token. Best effort: the
    /// account is gone from the launcher either way, so a server that cannot
    /// be reached within a few seconds is left alone.
    pub(super) async fn sign_out_third_party(&self, key: &str, entry: &AccountEntry) {
        let (Some(server), Ok(Some(stored))) = (
            entry.server.as_deref(),
            StoredYggdrasil::load(self.credentials.as_ref(), key),
        ) else {
            return;
        };
        let client = YggdrasilClient::new(&self.transport, server);
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            client.invalidate(&Secret::new(stored.access_token), &stored.client_token),
        )
        .await;
    }

    /// Once per run, looks in the background for a newer authlib-injector.
    /// Nothing waits for it and a failure changes nothing: the jar on disk
    /// keeps working.
    fn check_injector_update(&self, chain: &crate::transfer::SourceChain, dir: &std::path::Path) {
        if self.injector_checked.swap(true, Ordering::SeqCst) {
            return;
        }
        let (transport, chain, dir) = (self.transport.clone(), chain.clone(), dir.to_owned());
        tokio::spawn(async move {
            let _ = injector::refresh(&transport, &chain, &dir).await;
        });
    }

    /// A third-party account's session for a launch: the stored token while
    /// the server still accepts it, else a renewed one (`force` renews
    /// regardless), together with the agent that points the game at the server.
    pub(super) async fn third_party_session(
        &self,
        entry: &AccountEntry,
        force: bool,
    ) -> Result<AuthSession, ServiceError> {
        debug_assert_eq!(entry.kind, AccountKind::ThirdParty);
        let _one_at_a_time = self.auth_lock.lock().await;
        let key = entry.key();
        let (Some(server), Ok(profile_id)) = (entry.server.clone(), entry.profile_id()) else {
            return Err(ServiceError::SignInRequired(entry.name.clone()));
        };
        let Some(mut stored) = StoredYggdrasil::load(self.credentials.as_ref(), &key)? else {
            self.flag_sign_in(&key).await;
            return Err(ServiceError::SignInRequired(entry.name.clone()));
        };
        let client = YggdrasilClient::new(&self.transport, &server);
        let mut name = entry.name.clone();
        let token = Secret::new(stored.access_token.clone());
        let accepted = !force && client.validate(&token, &stored.client_token).await?;
        if !accepted {
            let renewed = match client.refresh(&token, &stored.client_token, None).await {
                Ok(session) => session,
                Err(error) if error.is_forbidden() => {
                    self.flag_sign_in(&key).await;
                    return Err(ServiceError::SignInRequired(entry.name.clone()));
                }
                Err(error) => return Err(error.into()),
            };
            let Some(selected) = renewed.selected.clone() else {
                self.flag_sign_in(&key).await;
                return Err(ServiceError::SignInRequired(entry.name.clone()));
            };
            if selected.id != profile_id {
                // Another character than the one this account was added for:
                // do not play as someone else.
                self.flag_sign_in(&key).await;
                return Err(ServiceError::SignInRequired(entry.name.clone()));
            }
            if selected.name.trim().is_empty() {
                return Err(
                    YggdrasilError::Malformed("the character has no name".to_owned()).into(),
                );
            }
            stored = StoredYggdrasil {
                client_token: renewed.client_token.clone(),
                access_token: renewed.access_token.expose().to_owned(),
                user_properties: renewed.user_properties.clone(),
            };
            stored.save(self.credentials.as_ref(), &key)?;
            if selected.name != entry.name {
                // A renamed character shows its new name.
                let _ = self.settings.lock().await.sign_in_third_party(
                    &server,
                    profile_id,
                    &selected.name,
                    entry.login.as_deref().unwrap_or_default(),
                );
            }
            name = selected.name;
        }
        let chain = self.chain().await?;
        let dir = self.layout.injector();
        self.check_injector_update(&chain, &dir);
        let (metadata, jar) = tokio::try_join!(
            async {
                fetch_metadata(&self.transport, &server)
                    .await
                    .map_err(ServiceError::from)
            },
            async {
                injector::ensure(&self.transport, &chain, &dir)
                    .await
                    .map_err(ServiceError::from)
            },
        )?;
        Ok(AuthSession::third_party(
            &name,
            profile_id,
            &stored.access_token,
            &stored.user_properties,
            Injection {
                jar: jar.path,
                api_root: server,
                prefetched: Some(metadata),
            },
        ))
    }
}
