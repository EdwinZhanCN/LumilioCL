use std::collections::BTreeMap;
use std::path::PathBuf;

use lumilio_plugin_api::{
    FetchResponse, HostContext, Manifest, PluginError, PluginState, SettingValue,
};

use super::access;

pub(super) struct Context {
    values: BTreeMap<String, SettingValue>,
    manifest: Manifest,
    game_dir: Option<PathBuf>,
}

impl Context {
    pub(super) fn new(manifest: &Manifest, state: &PluginState, game_dir: Option<PathBuf>) -> Self {
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
            values,
            manifest: manifest.clone(),
            game_dir,
        }
    }
}

impl HostContext for Context {
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
    fn fetch(&self, _url: &str) -> Result<FetchResponse, PluginError> {
        Err(PluginError::PermissionDenied)
    }
}
