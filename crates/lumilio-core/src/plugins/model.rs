//! A plugin asks for a 3D preview by naming a file in its view; the host asks
//! the plugin for the bytes the viewer should get (`InstanceTab::model`).

use std::path::PathBuf;

use lumilio_plugin_api::PluginError;

use super::{PluginHost, PluginStatus};

/// The most a plugin may hand the viewer.
const MAX_MODEL_BYTES: usize = 128 * 1024 * 1024;

impl PluginHost {
    /// What the viewer gets for `file` (relative to the game directory): the
    /// plugin's own `model`, which reads through the host and so only below
    /// its `ReadGameFiles` grant. A bad file fails this call, not the plugin;
    /// a panic or a timeout still does.
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
        // The plugin's own error rides inside `Ok`, so it is not taken for a failure.
        let result = self
            .call_in(plugin, Some(game_dir), move |plugin, ctx| {
                Ok(plugin
                    .instance_tab()
                    .ok_or_else(|| {
                        PluginError::Unavailable("the plugin has no instance tab".into())
                    })
                    .and_then(|tab| tab.model(ctx, &file)))
            })
            .await
            .ok_or_else(|| PluginError::Unavailable("the plugin is off or failed".into()))??;
        if result.len() > MAX_MODEL_BYTES {
            return Err(PluginError::Unavailable("the model is too large".into()));
        }
        Ok(result)
    }
}
