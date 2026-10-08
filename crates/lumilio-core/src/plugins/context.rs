use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use lumilio_plugin_api::{
    FetchRequest, FetchResponse, HostContext, Manifest, PluginError, PluginState, SettingValue,
};

use super::access;

pub(super) struct Context {
    pub(super) native: Option<super::native::Access>,
    pub(super) launch_id: Option<u64>,
    pub(super) revision: u64,
    values: BTreeMap<String, SettingValue>,
    manifest: Manifest,
    game_dir: Option<PathBuf>,
    network: Option<super::network::NetworkContext>,
    locale: Arc<str>,
}

impl Context {
    pub(super) fn new(
        manifest: &Manifest,
        state: &PluginState,
        game_dir: Option<PathBuf>,
        network: Option<super::network::NetworkContext>,
        locale: Arc<str>,
    ) -> Self {
        let values = manifest
            .settings
            .iter()
            .map(|field| {
                let value = state
                    .values
                    .get(&field.key)
                    .filter(|value| field.kind.accepts(value))
                    .cloned()
                    .unwrap_or_else(|| field.kind.default_value());
                (field.key.clone(), value)
            })
            .collect();
        Self {
            native: None,
            launch_id: None,
            revision: 0,
            values,
            manifest: manifest.clone(),
            game_dir,
            network,
            locale,
        }
    }
}

impl HostContext for Context {
    fn locale(&self) -> &str {
        &self.locale
    }
    fn launch_id(&self) -> Option<u64> {
        self.launch_id
    }
    fn settings_revision(&self) -> u64 {
        self.revision
    }
    fn discord_activity(
        &self,
        activity: Option<lumilio_plugin_api::DiscordActivity>,
    ) -> Result<(), PluginError> {
        self.native
            .as_ref()
            .ok_or(PluginError::PermissionDenied)?
            .update(activity)
    }
    fn setting(&self, key: &str) -> Option<SettingValue> {
        self.values.get(key).cloned()
    }
    fn read_file(&self, path: &str) -> Result<Vec<u8>, PluginError> {
        let game_dir = self
            .game_dir
            .as_ref()
            .ok_or(PluginError::PermissionDenied)?;
        access::read(game_dir, &self.manifest, path)
    }
    fn list_files(&self, dir: &str) -> Result<Vec<String>, PluginError> {
        let game_dir = self
            .game_dir
            .as_ref()
            .ok_or(PluginError::PermissionDenied)?;
        access::list(game_dir, &self.manifest, dir)
    }
    fn fetch(&self, url: &str) -> Result<FetchResponse, PluginError> {
        self.request(FetchRequest::get(url))
    }
    fn request(&self, request: FetchRequest) -> Result<FetchResponse, PluginError> {
        self.network
            .as_ref()
            .ok_or(PluginError::PermissionDenied)?
            .fetch(&self.manifest, request)
    }
}
