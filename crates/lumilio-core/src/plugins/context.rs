use std::collections::BTreeMap;

use lumilio_plugin_api::{
    FetchResponse, HostContext, Manifest, PluginError, PluginState, SettingValue,
};

pub(super) struct Context {
    values: BTreeMap<String, SettingValue>,
}

impl Context {
    pub(super) fn new(manifest: &Manifest, state: &PluginState) -> Self {
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
        Self { values }
    }
}

impl HostContext for Context {
    fn setting(&self, key: &str) -> Option<SettingValue> {
        self.values.get(key).cloned()
    }
    fn read_file(&self, _path: &str) -> Result<Vec<u8>, PluginError> {
        Err(PluginError::PermissionDenied)
    }
    fn fetch(&self, _url: &str) -> Result<FetchResponse, PluginError> {
        Err(PluginError::PermissionDenied)
    }
}
