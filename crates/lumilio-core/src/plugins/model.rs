//! A plugin asks for a 3D preview by naming a file in its view; the host reads
//! the bytes itself, under the same rules as `HostContext::read_file`.

use std::path::PathBuf;

use lumilio_plugin_api::PluginError;

use super::{PluginHost, PluginStatus, access};

impl PluginHost {
    /// The bytes of `file` (relative to the game directory) for a plugin that
    /// is on and whose `ReadGameFiles` grant covers it.
    pub async fn read_model(
        &self,
        plugin: &str,
        game_dir: PathBuf,
        file: &str,
    ) -> Result<Vec<u8>, PluginError> {
        let info = self
            .list()
            .await
            .into_iter()
            .find(|info| info.manifest.id == plugin)
            .ok_or_else(|| PluginError::InvalidInput(format!("unknown plugin {plugin}")))?;
        if info.status != PluginStatus::Enabled {
            return Err(PluginError::Unavailable("the plugin is off".into()));
        }
        let (manifest, file) = (info.manifest, file.to_owned());
        tokio::task::spawn_blocking(move || access::read(&game_dir, &manifest, &file))
            .await
            .map_err(|error| PluginError::Unavailable(error.to_string()))?
    }
}
