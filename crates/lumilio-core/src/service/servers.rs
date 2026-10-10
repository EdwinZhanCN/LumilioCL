use super::LauncherService;
use super::error::ServiceError;
use crate::servers::{self, ServerEntry, ServerStatus};
use crate::transfer::Transport;

impl<T: Transport + Clone> LauncherService<T> {
    /// The instance's saved servers. Reading needs no instance lease.
    pub async fn servers(&self, id: &str) -> Result<Vec<ServerEntry>, ServiceError> {
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        tokio::task::spawn_blocking(move || servers::list(&game_dir))
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::from)
    }

    /// Adds a server at the end of the list. Takes the instance lease.
    pub async fn add_server(&self, id: &str, entry: ServerEntry) -> Result<(), ServiceError> {
        self.edit_servers(id, move |dir| servers::add(dir, &entry))
            .await
    }

    /// Replaces the server at `index`, which must still be `expected`.
    pub async fn update_server(
        &self,
        id: &str,
        index: usize,
        expected: ServerEntry,
        entry: ServerEntry,
    ) -> Result<(), ServiceError> {
        self.edit_servers(id, move |dir| {
            servers::update(dir, index, &expected, &entry)
        })
        .await
    }

    pub async fn remove_server(
        &self,
        id: &str,
        index: usize,
        expected: ServerEntry,
    ) -> Result<(), ServiceError> {
        self.edit_servers(id, move |dir| servers::remove(dir, index, &expected))
            .await
    }

    pub async fn move_server(
        &self,
        id: &str,
        index: usize,
        expected: ServerEntry,
        to: usize,
    ) -> Result<(), ServiceError> {
        self.edit_servers(id, move |dir| servers::move_to(dir, index, &expected, to))
            .await
    }

    async fn edit_servers(
        &self,
        id: &str,
        edit: impl FnOnce(&std::path::Path) -> Result<(), servers::ServerError> + Send + 'static,
    ) -> Result<(), ServiceError> {
        let _lease = self.reserve_instance(id)?;
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        tokio::task::spawn_blocking(move || edit(&game_dir))
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::from)
    }

    /// Asks a server how it is, with the handshake the instance's game version
    /// calls for (the legacy ping for a pre-1.7 version). A server that does
    /// not answer is an error the caller shows as "offline".
    pub async fn server_status(
        &self,
        id: &str,
        address: &str,
    ) -> Result<ServerStatus, ServiceError> {
        let record = self.instance(id).await?;
        let versions = self.layout.versions();
        let game_version = record.game_version;
        let protocol = tokio::task::spawn_blocking(move || {
            servers::protocol_version(&game_version, &versions)
        })
        .await
        .map_err(std::io::Error::other)?;
        servers::probe(address, protocol)
            .await
            .map_err(ServiceError::from)
    }
}
