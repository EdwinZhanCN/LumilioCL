use tokio::sync::mpsc;

use super::LauncherService;
use crate::activity::CancellationToken;
use crate::error::ServiceError;
use crate::instances::{InstanceRecord, Loader};
use crate::launching::{GameExit, LaunchParts, LaunchUpdate};
use crate::transfer::Transport;

impl<T: Transport + Clone> LauncherService<T> {
    /// Launching and installing, bound to what they read and write.
    pub(crate) fn launch_parts(&self) -> LaunchParts<'_, T> {
        LaunchParts {
            state: &self.launching,
            instances: &self.instances,
            context: &self.context,
            layout: &self.layout,
            plugins: &self.plugins,
            network: &self.network,
            settings: &self.settings,
            tasks: &self.tasks,
            java: &self.java,
            accounts: &self.accounts,
            appearance: &self.appearance,
            game_settings: &self.game_settings,
        }
    }

    /// Lets the next launch of `id` pick its Java automatically, once,
    /// whatever its settings say. Any launch of it uses this up.
    pub fn launch_next_with_automatic_java(&self, id: &str) {
        self.launching.automatic_java_once(id);
    }

    /// Prepares and starts an instance, recording the session afterwards.
    pub async fn launch(
        &self,
        id: &str,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<GameExit, ServiceError> {
        self.launch_parts().launch(id, None, updates, cancel).await
    }

    /// Starts the game and goes straight into a saved world. The instance's
    /// own direct-start choice is not changed.
    pub async fn launch_world(
        &self,
        id: &str,
        world: &str,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<GameExit, ServiceError> {
        self.launch_parts()
            .launch_world(id, world, updates, cancel)
            .await
    }

    /// Starts the game and goes straight onto a multiplayer server. The
    /// instance's own direct-start choice is not changed.
    pub async fn launch_server(
        &self,
        id: &str,
        address: &str,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<GameExit, ServiceError> {
        self.launch_parts()
            .launch_server(id, address, updates, cancel)
            .await
    }

    /// Installs an instance's game files without starting it. The task shows
    /// in Activity and `cancel_task` can stop it. `installed` becomes true only
    /// when the whole install finished; a failure or cancel leaves it false.
    pub async fn install_instance(
        &self,
        id: &str,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<(), ServiceError> {
        self.launch_parts()
            .install_instance(id, updates, cancel)
            .await
    }

    /// Checks every game file of an instance against its release and fetches
    /// whatever is missing or damaged again. Worlds, mods and settings are
    /// never touched.
    pub async fn repair_instance(
        &self,
        id: &str,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<(), ServiceError> {
        self.launch_parts()
            .repair_instance(id, updates, cancel)
            .await
    }

    /// Changes the game version and/or the loader of an instance, committing
    /// the new combination only once it is fully prepared.
    pub async fn change_runtime(
        &self,
        id: &str,
        game_version: &str,
        loader: Loader,
        loader_version: Option<&str>,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        self.launch_parts()
            .change_runtime(id, game_version, loader, loader_version, updates, cancel)
            .await
    }
}
