//! Account-bound appearance operations and the launcher's local wardrobe.
//! Disk work stays on blocking workers. No token enters the library index.

use super::{LauncherService, ServiceError};
use crate::{
    activity::CancellationToken,
    microsoft::Secret,
    settings::{AccountEntry, AccountKind, SettingsError},
    skin::{
        self, AccountLook, AppearanceChange, AppearanceError, AppearanceUpdate, LibrarySkin,
        MojangClient, MojangProfile, PairedCape, Pixels, SkinLibrary, SkinModel, SkinSource,
    },
    transfer::Transport,
};
use std::{path::PathBuf, time::Duration};

/// Our initial delay before one explicit confirmation read (ADR 0039). This
/// does not guarantee that Mojang's profile has propagated yet.
const PROPAGATION_DELAY: Duration = Duration::from_secs(11);

pub struct AppearanceApplyResult {
    pub update: Option<AppearanceUpdate>,
    pub cape_failure: Option<(Option<String>, ServiceError)>,
}

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

    pub(super) async fn appearance_token(
        &self,
        key: &str,
    ) -> Result<(AccountEntry, Secret), ServiceError> {
        let entry = self.appearance_account(key).await?;
        if entry.kind != AccountKind::Microsoft {
            return Err(AppearanceError::NotMicrosoft.into());
        }
        let session = self.microsoft_session(&entry, false).await?;
        Ok((entry, Secret::new(session.access_token())))
    }

    pub(super) async fn appearance_answer<R>(
        &self,
        key: &str,
        result: Result<R, AppearanceError>,
    ) -> Result<R, ServiceError> {
        if matches!(result, Err(AppearanceError::SignInRequired)) {
            self.flag_sign_in(key).await;
        }
        result.map_err(Into::into)
    }

    /// Success means Mojang accepted the operation. A stale or absent profile
    /// in the response is advisory, never grounds for repeating the write.
    pub async fn change_account_appearance(
        &self,
        key: &str,
        change: AppearanceChange,
    ) -> Result<AppearanceUpdate, ServiceError> {
        if let AppearanceChange::Cape(Some(id)) = &change
            && !self
                .account_profile(key)
                .await?
                .capes
                .iter()
                .any(|cape| &cape.id == id)
        {
            return Err(AppearanceError::CapeNotOwned.into());
        }
        let slot = self.profiles.slot(key).await;
        let mut state = slot.lock().await;
        let (entry, token) = self.appearance_token(key).await?;
        state.token(&token);
        if let Some(error) = state.blocked() {
            return Err(error.into());
        }
        let result = MojangClient::new(&self.transport)
            .change_validated(&token, change.clone())
            .await;
        if !self.profiles.is_current(key, &slot).await {
            return Err(ServiceError::Cancelled);
        }
        match &result {
            Ok(update) => {
                state.failure = None;
                if let Some(profile) = &update.profile
                    && super::appearance::validate_profile(&entry, profile).is_ok()
                    && reply_matches_change(
                        &change,
                        profile,
                        state.profile.as_ref().map(|(_, profile)| profile),
                    )
                {
                    state.profile = Some((tokio::time::Instant::now(), profile.clone()));
                }
            }
            Err(error) => state.failed_write(error),
        }
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
                super::error::read_unless_cancelled(&cancel, self.refresh_account_profile(key)).await
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
        let png = MojangClient::cached(&self.transport, &self.textures)
            .skin_png(skin)
            .await?;
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
    ) -> Result<AppearanceApplyResult, ServiceError> {
        let entry = self.appearance_account(key).await?;
        if entry.kind == AccountKind::ThirdParty {
            return Err(AppearanceError::NotMicrosoft.into());
        }
        let paired = self
            .with_skin_library({
                let id = id.clone();
                move |library| {
                    Ok(library
                        .entries()
                        .iter()
                        .find(|entry| entry.id == id)
                        .map(|entry| entry.cape.clone())
                        .unwrap_or_default())
                }
            })
            .await?;
        let (path, model, png) = self.library_skin(id).await?;
        match entry.kind {
            AccountKind::Microsoft => {
                let update = self
                    .change_account_appearance(key, AppearanceChange::Upload { png, model })
                    .await?;
                // The skin's paired cape follows it; "keep" leaves the cape.
                let cape = match paired {
                    PairedCape::Keep => {
                        return Ok(AppearanceApplyResult {
                            update: Some(update),
                            cape_failure: None,
                        });
                    }
                    PairedCape::Hidden => None,
                    PairedCape::Cape(id) => Some(id),
                };
                let cape_failure = self
                    .change_account_appearance(key, AppearanceChange::Cape(cape.clone()))
                    .await
                    .err()
                    .map(|error| (cape, error));
                Ok(AppearanceApplyResult {
                    update: Some(update),
                    cape_failure,
                })
            }
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
                Ok(AppearanceApplyResult {
                    update: None,
                    cape_failure: None,
                })
            }
            AccountKind::ThirdParty => unreachable!("third-party accounts are read-only"),
        }
    }

    /// Applies edited library values without changing the stored entry.
    pub async fn wear_library_skin_draft(
        &self,
        key: &str,
        id: String,
        model: SkinModel,
        cape: PairedCape,
        replacement: Option<PathBuf>,
    ) -> Result<AppearanceApplyResult, ServiceError> {
        let entry = self.appearance_account(key).await?;
        if entry.kind == AccountKind::ThirdParty {
            return Err(AppearanceError::NotMicrosoft.into());
        }
        let (path, _, library_png) = self.library_skin(id).await?;
        let selected_path = replacement.clone().unwrap_or(path);
        let png = match replacement {
            Some(path) => read_skin_file(path).await?,
            None => library_png,
        };
        match entry.kind {
            AccountKind::Microsoft => {
                let update = self
                    .change_account_appearance(key, AppearanceChange::Upload { png, model })
                    .await?;
                let cape = match cape {
                    PairedCape::Keep => {
                        return Ok(AppearanceApplyResult {
                            update: Some(update),
                            cape_failure: None,
                        });
                    }
                    PairedCape::Hidden => None,
                    PairedCape::Cape(id) => Some(id),
                };
                let cape_failure = self
                    .change_account_appearance(key, AppearanceChange::Cape(cape.clone()))
                    .await
                    .err()
                    .map(|error| (cape, error));
                Ok(AppearanceApplyResult {
                    update: Some(update),
                    cape_failure,
                })
            }
            AccountKind::Offline => {
                let cape_path = match entry.skin {
                    Some(skin::SkinChoice::Local { cape, .. }) => cape,
                    _ => None,
                };
                self.set_account_skin(
                    key,
                    Some(skin::SkinChoice::Local {
                        skin: Some(selected_path),
                        model,
                        cape: cape_path,
                    }),
                )
                .await?;
                Ok(AppearanceApplyResult {
                    update: None,
                    cape_failure: None,
                })
            }
            AccountKind::ThirdParty => unreachable!(),
        }
    }

    /// Reuploads the current texture with a different arm model.
    pub async fn change_account_skin_model(
        &self,
        key: &str,
        model: SkinModel,
    ) -> Result<Option<AppearanceUpdate>, ServiceError> {
        let profile = self.account_profile(key).await?;
        let Some(skin) = profile.active_skin() else {
            return Ok(None);
        };
        if skin.model == model {
            return Ok(None);
        }
        let png = MojangClient::cached(&self.transport, &self.textures)
            .skin_png(skin)
            .await?;
        self.change_account_appearance(key, AppearanceChange::Upload { png, model })
            .await
            .map(Some)
    }

    pub async fn upload_account_skin_file(
        &self,
        key: &str,
        path: PathBuf,
        model: SkinModel,
    ) -> Result<AppearanceUpdate, ServiceError> {
        let png = read_skin_file(path).await?;
        self.change_account_appearance(key, AppearanceChange::Upload { png, model })
            .await
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

    pub async fn rename_library_skin(&self, id: String, name: String) -> Result<(), ServiceError> {
        self.with_skin_library(move |library| library.rename(&id, &name))
            .await
    }

    pub async fn set_library_skin_cape(
        &self,
        id: String,
        cape: PairedCape,
    ) -> Result<(), ServiceError> {
        self.with_skin_library(move |library| library.set_cape(&id, cape))
            .await
    }

    /// Puts the picture at `path` in an entry's place (see
    /// `SkinLibrary::replace`); the same size limit as an import applies.
    pub async fn replace_library_skin(
        &self,
        id: String,
        path: PathBuf,
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
            library.replace(&id, &bytes, SkinSource::LocalFile(path))
        })
        .await
    }

    /// Every library skin as preview pixels, for the wardrobe's pictures.
    /// A picture that no longer decodes is left out rather than failing all.
    pub async fn library_looks(&self) -> Result<Vec<(String, AccountLook)>, ServiceError> {
        self.with_skin_library(|library| {
            Ok(library
                .entries()
                .iter()
                .filter_map(|entry| {
                    let png = library.picture(&entry.id).ok()?;
                    let skin = skin::skin_pixels(&png).ok()?;
                    Some((
                        entry.id.clone(),
                        AccountLook {
                            model: entry.model,
                            skin: Some(skin),
                            cape: None,
                        },
                    ))
                })
                .collect())
        })
        .await
    }

    /// The pictures of the capes a Microsoft account owns, by cape id. A
    /// cape whose picture cannot be read is left out.
    pub async fn owned_cape_pictures(&self, profile: &MojangProfile) -> Vec<(String, Pixels)> {
        let client = MojangClient::cached(&self.transport, &self.textures);
        let mut pictures = Vec::new();
        for cape in &profile.capes {
            if let Ok(png) = client.cape_png(cape).await
                && let Ok(pixels) = skin::cape_pixels(&png)
            {
                pictures.push((cape.id.clone(), pixels));
            }
        }
        pictures
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
        let remote = MojangClient::cached(&self.transport, &self.textures)
            .skin_png(skin)
            .await?;
        Ok(skin::skin_pixels(&png).map_err(ServiceError::Skin)?
            == skin::skin_pixels(&remote).map_err(ServiceError::Skin)?)
    }
}

fn reply_matches_change(
    change: &AppearanceChange,
    reply: &MojangProfile,
    before: Option<&MojangProfile>,
) -> bool {
    match change {
        AppearanceChange::DefaultSkin => reply.active_skin().is_none(),
        AppearanceChange::Cape(id) => reply.active_cape().map(|cape| &cape.id) == id.as_ref(),
        AppearanceChange::Upload { model, .. } => reply.active_skin().is_some_and(|skin| {
            skin.model == *model && before.and_then(MojangProfile::active_skin) != Some(skin)
        }),
    }
}

async fn read_skin_file(path: PathBuf) -> Result<Vec<u8>, ServiceError> {
    tokio::task::spawn_blocking(move || {
        if std::fs::metadata(&path)
            .map_err(|error| AppearanceError::Storage(error.to_string()))?
            .len()
            > 2 * 1024 * 1024
        {
            return Err(AppearanceError::Picture("skin exceeds size limit".into()));
        }
        std::fs::read(path).map_err(|error| AppearanceError::Storage(error.to_string()))
    })
    .await
    .map_err(std::io::Error::other)?
    .map_err(Into::into)
}
