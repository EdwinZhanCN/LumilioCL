use std::path::PathBuf;

use super::LauncherService;
use crate::activity::CancellationToken;
use crate::appearance::skin::{
    AccountLook, AppearanceChange, AppearanceUpdate, LibrarySkin, MojangProfile, PairedCape,
    Pixels, SkinChoice, SkinModel,
};
use crate::appearance::{AppearanceApplyResult, ProfileSnapshot};
use crate::error::ServiceError;
use crate::transfer::Transport;

impl<T: Transport + Clone> LauncherService<T> {
    pub async fn account_profile(&self, key: &str) -> Result<MojangProfile, ServiceError> {
        self.appearance.account_profile(&self.accounts, key).await
    }

    pub async fn account_profile_snapshot(
        &self,
        key: &str,
    ) -> Result<ProfileSnapshot, ServiceError> {
        self.appearance
            .account_profile_snapshot(&self.accounts, key)
            .await
    }

    pub async fn refresh_account_profile(&self, key: &str) -> Result<MojangProfile, ServiceError> {
        self.appearance
            .refresh_account_profile(&self.accounts, key)
            .await
    }

    pub async fn set_account_skin(
        &self,
        key: &str,
        skin: Option<SkinChoice>,
    ) -> Result<(), ServiceError> {
        self.appearance.set_account_skin(key, skin).await
    }

    pub async fn account_look(&self, key: &str) -> Result<AccountLook, ServiceError> {
        self.appearance.account_look(&self.accounts, key).await
    }

    pub async fn account_face(&self, key: &str) -> Result<Option<Pixels>, ServiceError> {
        self.appearance.account_face(&self.accounts, key).await
    }

    pub async fn change_account_appearance(
        &self,
        key: &str,
        change: AppearanceChange,
    ) -> Result<AppearanceUpdate, ServiceError> {
        self.appearance
            .change_account_appearance(&self.accounts, key, change)
            .await
    }

    pub async fn confirm_account_appearance(
        &self,
        key: &str,
        cancel: CancellationToken,
    ) -> Result<MojangProfile, ServiceError> {
        self.appearance
            .confirm_account_appearance(&self.accounts, key, cancel)
            .await
    }

    pub async fn skin_library(&self) -> Result<Vec<LibrarySkin>, ServiceError> {
        self.appearance.skin_library().await
    }

    pub async fn save_account_skin(&self, key: &str) -> Result<LibrarySkin, ServiceError> {
        self.appearance.save_account_skin(&self.accounts, key).await
    }

    pub async fn wear_library_skin(
        &self,
        key: &str,
        id: String,
    ) -> Result<AppearanceApplyResult, ServiceError> {
        self.appearance
            .wear_library_skin(&self.accounts, key, id)
            .await
    }

    pub async fn wear_library_skin_draft(
        &self,
        key: &str,
        id: String,
        model: SkinModel,
        cape: PairedCape,
        replacement: Option<PathBuf>,
    ) -> Result<AppearanceApplyResult, ServiceError> {
        self.appearance
            .wear_library_skin_draft(&self.accounts, key, id, model, cape, replacement)
            .await
    }

    pub async fn change_account_skin_model(
        &self,
        key: &str,
        model: SkinModel,
    ) -> Result<Option<AppearanceUpdate>, ServiceError> {
        self.appearance
            .change_account_skin_model(&self.accounts, key, model)
            .await
    }

    pub async fn upload_account_skin_file(
        &self,
        key: &str,
        path: PathBuf,
        model: SkinModel,
    ) -> Result<AppearanceUpdate, ServiceError> {
        self.appearance
            .upload_account_skin_file(&self.accounts, key, path, model)
            .await
    }

    pub async fn import_library_skin(
        &self,
        path: PathBuf,
        model: Option<SkinModel>,
    ) -> Result<LibrarySkin, ServiceError> {
        self.appearance.import_library_skin(path, model).await
    }

    pub async fn rename_library_skin(&self, id: String, name: String) -> Result<(), ServiceError> {
        self.appearance.rename_library_skin(id, name).await
    }

    pub async fn set_library_skin_cape(
        &self,
        id: String,
        cape: PairedCape,
    ) -> Result<(), ServiceError> {
        self.appearance.set_library_skin_cape(id, cape).await
    }

    pub async fn replace_library_skin(
        &self,
        id: String,
        path: PathBuf,
    ) -> Result<LibrarySkin, ServiceError> {
        self.appearance.replace_library_skin(id, path).await
    }

    pub async fn library_looks(&self) -> Result<Vec<(String, AccountLook)>, ServiceError> {
        self.appearance.library_looks().await
    }

    pub async fn owned_cape_pictures(&self, profile: &MojangProfile) -> Vec<(String, Pixels)> {
        self.appearance.owned_cape_pictures(profile).await
    }

    pub async fn reorder_library_skins(&self, ids: Vec<String>) -> Result<(), ServiceError> {
        self.appearance.reorder_library_skins(ids).await
    }

    pub async fn set_library_skin_model(
        &self,
        id: String,
        model: SkinModel,
    ) -> Result<(), ServiceError> {
        self.appearance.set_library_skin_model(id, model).await
    }

    pub async fn remove_library_skin(&self, id: String) -> Result<(), ServiceError> {
        self.appearance.remove_library_skin(id).await
    }

    pub async fn library_skin(
        &self,
        id: String,
    ) -> Result<(PathBuf, SkinModel, Vec<u8>), ServiceError> {
        self.appearance.library_skin(id).await
    }

    pub async fn profile_wears_library_skin(
        &self,
        profile: &MojangProfile,
        id: String,
    ) -> Result<bool, ServiceError> {
        self.appearance
            .profile_wears_library_skin(profile, id)
            .await
    }
}
