//! An inline 3D preview's schematic and its own game's resource pack (ADR 0028).

use super::{LauncherService, ServiceError};
use crate::model_assets::{ResourcePack, build_pack};
use crate::transfer::Transport;
use std::path::PathBuf;

/// Everything the viewer is served.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelPreview {
    pub schematic: Vec<u8>,
    /// `None` when the game is not installed, so there is no jar to draw from.
    pub pack: Option<ResourcePack>,
}

/// The vanilla client jar an installed instance plays with.
fn client_jar(
    layout: &crate::layout::Layout,
    record: &crate::instance::InstanceRecord,
) -> Option<PathBuf> {
    let directories = layout.launch_directories(record);
    let id = record.release_id()?;
    let manifest =
        std::fs::read_to_string(directories.versions().join(&id).join(format!("{id}.json")))
            .ok()
            .and_then(|text| crate::release::ReleaseManifest::decode_json(&text).ok())?;
    let jar_id = manifest.jar_id().to_owned();
    let jar = directories
        .versions()
        .join(&jar_id)
        .join(format!("{jar_id}.jar"));
    jar.is_file().then_some(jar)
}

impl<T: Transport + Clone> LauncherService<T> {
    /// The schematic `file` (a path the plugin may read) and the resource pack
    /// for this instance. Without a readable jar the UI explains why it cannot preview.
    pub async fn plugin_model(
        &self,
        instance: &str,
        plugin: &str,
        file: &str,
    ) -> Result<ModelPreview, ServiceError> {
        let record = self.instance(instance).await?;
        let schematic = self
            .plugins
            .read_model(plugin, self.layout.game(instance), file)
            .await
            .map_err(ServiceError::Plugin)?;
        let layout = self.layout.clone();
        let pack = tokio::task::spawn_blocking(move || {
            let jar = client_jar(&layout, &record)?;
            build_pack(&jar).ok()
        })
        .await
        .map_err(std::io::Error::other)?;
        Ok(ModelPreview { schematic, pack })
    }
}
