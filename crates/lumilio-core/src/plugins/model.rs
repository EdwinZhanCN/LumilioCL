//! A plugin names a file in its view; the host reads its unchanged bytes
//! through the same permissions as any other game file (ADR 0028).

use std::path::PathBuf;

use lumilio_plugin_api::PluginError;

use super::{PluginHost, PluginStatus};

impl PluginHost {
    /// What the viewer gets for `file` (relative to the game directory): the
    /// unchanged bytes, read under its `ReadGameFiles` grant. A bad file fails
    /// this preview only, not the plugin.
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
        let file = file.to_owned();
        // File errors ride inside `Ok`, so they do not fail the plugin itself.
        self.call_in(plugin, Some(game_dir), move |_, ctx| {
            Ok(ctx.read_file(&file))
        })
        .await
        .ok_or_else(|| PluginError::Unavailable("the plugin is off or failed".into()))?
    }
}
