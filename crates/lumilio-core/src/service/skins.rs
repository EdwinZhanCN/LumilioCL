//! How offline accounts look: choosing a skin, and what a launch does with it
//! (a skin server on this machine and the authlib-injector agent that points
//! the game at it). The rules follow HMCL's `OfflineAccount`
//! (`HMCLCore/.../auth/offline/OfflineAccount.java`, Copyright (C) 2020
//! huangyuhui and contributors, GPL-3.0-or-later; ADR 0011).

use std::sync::Arc;

use super::LauncherService;
use super::error::ServiceError;
use crate::account::{AuthSession, Injection, Keepalive, OfflineProfile};
use crate::injector;
use crate::settings::{AccountKind, SettingsError};
use crate::skin::{self, AccountLook, Character, LoadedSkin, LocalSkinServer, Signer, SkinChoice};
use crate::transfer::Transport;

impl<T: Transport + Clone> LauncherService<T> {
    /// Chooses how an offline account looks (`None` is the game's default
    /// skin). A choice that cannot work (a file that is not a picture, a
    /// bad address) is refused now rather than at launch.
    pub async fn set_account_skin(
        &self,
        key: &str,
        skin: Option<SkinChoice>,
    ) -> Result<(), ServiceError> {
        if let Some(choice) = skin.clone() {
            tokio::task::spawn_blocking(move || skin::check(&choice))
                .await
                .map_err(std::io::Error::other)?
                .map_err(ServiceError::Skin)?;
        }
        Ok(self.settings.lock().await.set_account_skin(key, skin)?)
    }

    /// What an account looks like, for the Accounts page's preview: an
    /// offline account's chosen skin and cape, decoded. The default look
    /// (no skin) is returned for an account without a choice, and for now
    /// for signed-in accounts.
    pub async fn account_look(&self, key: &str) -> Result<AccountLook, ServiceError> {
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
        let Some(choice) = entry.skin.filter(|_| entry.kind == AccountKind::Offline) else {
            return Ok(AccountLook::default());
        };
        let loaded = skin::load(&self.transport, &choice, &entry.name)
            .await
            .map_err(ServiceError::Skin)?;
        tokio::task::spawn_blocking(move || AccountLook::from_loaded(&loaded))
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::Skin)
    }

    /// The key the skin server signs with, made on first use (and kept in
    /// the launcher's folder, so later launches need no waiting).
    async fn skin_signer(&self) -> Result<Arc<Signer>, ServiceError> {
        let path = self.layout.root().join("skin-server.pem");
        let bits = self.skin_key_bits;
        self.skin_signer
            .get_or_try_init(|| async {
                tokio::task::spawn_blocking(move || {
                    Signer::load_or_create(&path, bits).map(Arc::new)
                })
                .await
                .map_err(std::io::Error::other)?
                .map_err(ServiceError::from)
            })
            .await
            .cloned()
    }

    /// An offline player's session with their skin: the skin is loaded, a
    /// server on this machine answers for the player, and the session carries
    /// the agent that points the game at it. A skin that cannot be loaded
    /// does not stop the game: it starts without, and says so.
    pub(super) async fn offline_skin_session(
        &self,
        profile: &OfflineProfile,
        choice: &SkinChoice,
    ) -> Result<AuthSession, ServiceError> {
        let mut notes = Vec::new();
        let skin = match skin::load(&self.transport, choice, profile.name()).await {
            Ok(skin) => skin,
            Err(error) => {
                notes.push(format!("the skin was not loaded: {error}"));
                LoadedSkin::default()
            }
        };
        let signer = self.skin_signer().await?;
        let chain = self.chain().await?;
        let jar = injector::ensure(&self.transport, &chain, &self.layout.injector()).await?;
        let server = LocalSkinServer::start(
            Character {
                id: profile.id(),
                name: profile.name().to_owned(),
                skin,
            },
            signer,
        )
        .await?;
        let injection = Injection {
            jar: jar.path,
            api_root: server.api_root(),
            prefetched: None,
        };
        Ok(profile
            .session()
            .with_injection(injection)
            .with_keepalive(Keepalive::new(server))
            .with_notes(notes))
    }
}
