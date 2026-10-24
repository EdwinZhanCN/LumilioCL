//! Shared native preferences, explicit instance overrides and unconfirmed game changes.
mod catalog;
mod document;
mod edit;
mod engine;
mod operations;
mod storage;

#[cfg(test)]
mod tests;

pub(crate) use catalog::valid;
pub use catalog::{GameSettingGroup, GameSettingKind, game_setting_group, game_setting_kind};
pub(crate) use edit::{edit_option, options};
pub(crate) use engine::{apply, capture, resolve, view};
pub(crate) use operations::GameSettings;
pub(crate) use storage::{binding, delete, list, new_preset, preset, save_binding, save_preset};
pub(super) use values::Values;
pub(crate) use values::{Binding, Snapshot, read_values};
pub use values::{
    GameSettingValue, GameSettingsError, GameSettingsView, InstanceOptions, OptionChange,
    OptionsFile, SettingChange, SettingResolution, SettingsPreset,
};

mod values;
