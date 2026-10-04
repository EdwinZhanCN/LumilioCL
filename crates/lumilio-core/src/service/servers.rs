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

    /// Asks a server how it is. Needs no instance; a server that does not
    /// answer is an error the caller shows as "offline".
    pub async fn server_status(&self, address: &str) -> Result<ServerStatus, ServiceError> {
        servers::probe(address).await.map_err(ServiceError::from)
    }
}
