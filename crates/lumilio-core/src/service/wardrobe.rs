//! Account-bound appearance operations and the launcher's local wardrobe.
//! Disk work stays on blocking workers. No token enters the library index.

use super::{LauncherService, ServiceError};
use crate::{
    activity::CancellationToken,
    microsoft::Secret,
    settings::{AccountEntry, AccountKind, SettingsError},
    skin::{
        self, AppearanceChange, AppearanceError, AppearanceUpdate, LibrarySkin, MojangClient,
        MojangProfile, SkinLibrary, SkinModel, SkinSource,
    },
    transfer::Transport,
};
use std::{path::PathBuf, time::Duration};

/// Modrinth waits eleven seconds before verifying changes in its profile
/// cache (`minecraft_skins/mod.rs`, GPL-3.0-only; ADR 0022).
const PROPAGATION_DELAY: Duration = Duration::from_secs(11);

impl<T: Transport + Clone> LauncherService<T> {
    async fn appearance_account(&self, key: &str) -> Result<AccountEntry, ServiceError> {
        self.settings
            .lock()
            .await
            .get()
            .accounts
            .iter()
            .find(|entry| entry.key() == key)
            .cloned()
            .ok_or_else(|| SettingsError::UnknownAccount(key.to_owned()).into())
    }

    async fn appearance_token(&self, key: &str) -> Result<(AccountEntry, Secret), ServiceError> {
        let entry = self.appearance_account(key).await?;
        if entry.kind != AccountKind::Microsoft {
            return Err(AppearanceError::NotMicrosoft.into());
        }
        let session = self.microsoft_session(&entry, false).await?;
        Ok((entry, Secret::new(session.access_token())))
    }

    async fn appearance_answer<R>(
        &self,
        key: &str,
        result: Result<R, AppearanceError>,
    ) -> Result<R, ServiceError> {
        if matches!(result, Err(AppearanceError::SignInRequired)) {
            self.flag_sign_in(key).await;
        }
        result.map_err(Into::into)
    }

    pub async fn account_profile(&self, key: &str) -> Result<MojangProfile, ServiceError> {
        let (entry, token) = self.appearance_token(key).await?;
        let profile = self
            .appearance_answer(
                key,
                MojangClient::new(&self.transport).profile(&token).await,
            )
            .await?;
        let expected = entry
            .profile_id()
            .map_err(|_| AppearanceError::Protocol("invalid account id".into()))?;
        let actual = crate::account::ProfileId::parse(&profile.id)
            .map_err(|_| AppearanceError::Protocol("invalid profile id".into()))?;
        if expected != actual {
            return Err(
                AppearanceError::Protocol("profile belongs to another account".into()).into(),
            );
        }
        Ok(profile)
    }

    /// Success means Mojang accepted the operation. A stale or absent profile
    /// in the response is advisory, never grounds for repeating the write.
    pub async fn change_account_appearance(
        &self,
        key: &str,
        change: AppearanceChange,
    ) -> Result<AppearanceUpdate, ServiceError> {
        let _one_at_a_time = self.wardrobe_lock.lock().await;
        let (_, token) = self.appearance_token(key).await?;
        let result = MojangClient::new(&self.transport)
            .change(&token, change)
            .await;
        self.appearance_answer(key, result).await
    }

    /// Read-only confirmation can be cancelled when the detail closes. A
    /// still-old profile is returned normally for the UI to show as pending.
    pub async fn confirm_account_appearance(
        &self,
        key: &str,
        cancel: CancellationToken,
    ) -> Result<MojangProfile, ServiceError> {
        tokio::select! {
            () = cancel.cancelled() => Err(ServiceError::Cancelled),
            () = tokio::time::sleep(PROPAGATION_DELAY) => {
                super::error::read_unless_cancelled(&cancel, self.account_profile(key)).await
            }
        }
    }

    async fn with_skin_library<R: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut SkinLibrary) -> Result<R, AppearanceError> + Send + 'static,
    ) -> Result<R, ServiceError> {
        let _one_at_a_time = self.wardrobe_lock.lock().await;
        let root = self.layout.root().to_owned();
        tokio::task::spawn_blocking(move || operation(&mut SkinLibrary::open(&root)?))
            .await
            .map_err(std::io::Error::other)?
            .map_err(Into::into)
    }

    pub async fn skin_library(&self) -> Result<Vec<LibrarySkin>, ServiceError> {
        self.with_skin_library(|library| Ok(library.entries().to_vec()))
            .await
    }

    /// Saves a normalized copy of the account's current skin, without
    /// changing the profile or downloading its cape.
    pub async fn save_account_skin(&self, key: &str) -> Result<LibrarySkin, ServiceError> {
        let profile = self.account_profile(key).await?;
        let skin = profile.active_skin().ok_or(AppearanceError::UnknownSkin)?;
        let png = MojangClient::new(&self.transport).skin_png(skin).await?;
        let model = skin.model;
        let source = SkinSource::Mojang(skin.url.clone());
        self.with_skin_library(move |library| library.add(profile.name, &png, model, source))
            .await
    }

    /// Microsoft wears the copied PNG through Mojang. Offline selection
    /// keeps the existing local cape and still uses ADR 0024 at launch.
    pub async fn wear_library_skin(
        &self,
        key: &str,
        id: String,
    ) -> Result<Option<AppearanceUpdate>, ServiceError> {
        let entry = self.appearance_account(key).await?;
        if entry.kind == AccountKind::ThirdParty {
            return Err(AppearanceError::NotMicrosoft.into());
        }
        let (path, model, png) = self.library_skin(id).await?;
        match entry.kind {
            AccountKind::Microsoft => self
                .change_account_appearance(key, AppearanceChange::Upload { png, model })
                .await
                .map(Some),
            AccountKind::Offline => {
                let cape = match entry.skin {
                    Some(skin::SkinChoice::Local { cape, .. }) => cape,
                    _ => None,
                };
                self.set_account_skin(
                    key,
                    Some(skin::SkinChoice::Local {
                        skin: Some(path),
                        model,
                        cape,
                    }),
                )
                .await?;
                Ok(None)
            }
            AccountKind::ThirdParty => unreachable!("third-party accounts are read-only"),
        }
    }

    pub async fn import_library_skin(
        &self,
        path: PathBuf,
        model: Option<SkinModel>,
    ) -> Result<LibrarySkin, ServiceError> {
        self.with_skin_library(move |library| {
            if std::fs::metadata(&path)
                .map_err(|error| AppearanceError::Storage(error.to_string()))?
                .len()
                > 2 * 1024 * 1024
            {
                return Err(AppearanceError::Picture("skin exceeds size limit".into()));
            }
            let bytes = std::fs::read(&path)
                .map_err(|error| AppearanceError::Storage(error.to_string()))?;
            let model = match model {
                Some(model) => model,
                None => {
                    let pixels = skin::skin_pixels(&bytes)
                        .map_err(|error| AppearanceError::Picture(error.to_string()))?;
                    if skin::looks_slim(&pixels) {
                        SkinModel::Slim
                    } else {
                        SkinModel::Wide
                    }
                }
            };
            let name = path
                .file_stem()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            library.add(name, &bytes, model, SkinSource::LocalFile(path))
        })
        .await
    }

    pub async fn reorder_library_skins(&self, ids: Vec<String>) -> Result<(), ServiceError> {
        self.with_skin_library(move |library| library.reorder(&ids))
            .await
    }
    pub async fn set_library_skin_model(
        &self,
        id: String,
        model: SkinModel,
    ) -> Result<(), ServiceError> {
        self.with_skin_library(move |library| library.set_model(&id, model))
            .await
    }
    pub async fn remove_library_skin(&self, id: String) -> Result<(), ServiceError> {
        self.with_skin_library(move |library| library.remove(&id))
            .await
    }

    /// Returns the copied file, model and bytes; the id is checked against
    /// the index before it can become a path.
    pub async fn library_skin(
        &self,
        id: String,
    ) -> Result<(PathBuf, SkinModel, Vec<u8>), ServiceError> {
        self.with_skin_library(move |library| {
            let model = library
                .entries()
                .iter()
                .find(|entry| entry.id == id)
                .ok_or(AppearanceError::UnknownSkin)?
                .model;
            Ok((library.path(&id)?, model, library.picture(&id)?))
        })
        .await
    }

    /// Wearing the same image may leave the CDN URL unchanged. Confirmation
    /// compares pixels and the arm model, not just profile metadata.
    pub async fn profile_wears_library_skin(
        &self,
        profile: &MojangProfile,
        id: String,
    ) -> Result<bool, ServiceError> {
        let Some(skin) = profile.active_skin() else {
            return Ok(false);
        };
        let (_, model, png) = self.library_skin(id).await?;
        if model != skin.model {
            return Ok(false);
        }
        let remote = MojangClient::new(&self.transport).skin_png(skin).await?;
        Ok(skin::skin_pixels(&png).map_err(ServiceError::Skin)?
            == skin::skin_pixels(&remote).map_err(ServiceError::Skin)?)
    }
}
