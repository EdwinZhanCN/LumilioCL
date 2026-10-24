use super::LauncherService;
use crate::accounts::ThirdPartySignIn;
use crate::accounts::account::ProfileId;
use crate::accounts::microsoft::DeviceCode;
use crate::accounts::yggdrasil::AuthServer;
use crate::activity::CancellationToken;
use crate::error::ServiceError;
use crate::transfer::Transport;

impl<T: Transport + Clone> LauncherService<T> {
    pub async fn add_account(&self, name: &str, uuid: Option<&str>) -> Result<(), ServiceError> {
        self.accounts.add_account(name, uuid).await
    }

    pub async fn select_account(&self, key: &str) -> Result<(), ServiceError> {
        self.accounts.select_account(key).await
    }

    pub async fn remove_account(&self, key: &str) -> Result<(), ServiceError> {
        self.accounts.remove_account(&self.appearance, key).await
    }

    pub async fn microsoft_sign_in(
        &self,
        on_code: impl FnOnce(&DeviceCode) + Send,
        cancel: CancellationToken,
    ) -> Result<(String, String), ServiceError> {
        self.accounts.microsoft_sign_in(on_code, cancel).await
    }

    pub async fn refresh_account(&self, key: &str) -> Result<(), ServiceError> {
        self.accounts.refresh_account(key).await
    }

    pub async fn auth_servers(&self) -> Vec<AuthServer> {
        self.accounts.auth_servers().await
    }

    pub async fn locate_auth_server(&self, address: &str) -> Result<AuthServer, ServiceError> {
        self.accounts.locate_auth_server(address).await
    }

    pub async fn add_auth_server(&self, server: &AuthServer) -> Result<(), ServiceError> {
        self.accounts.add_auth_server(server).await
    }

    pub async fn remove_auth_server(&self, url: &str) -> Result<(), ServiceError> {
        self.accounts.remove_auth_server(url).await
    }

    pub async fn third_party_sign_in(
        &self,
        server: &str,
        login: &str,
        password: &str,
    ) -> Result<ThirdPartySignIn, ServiceError> {
        self.accounts
            .third_party_sign_in(server, login, password)
            .await
    }

    pub async fn third_party_choose(
        &self,
        pending: u64,
        character: ProfileId,
    ) -> Result<(String, String), ServiceError> {
        self.accounts.third_party_choose(pending, character).await
    }

    pub fn third_party_abandon(&self, pending: u64) {
        self.accounts.third_party_abandon(pending)
    }
}
