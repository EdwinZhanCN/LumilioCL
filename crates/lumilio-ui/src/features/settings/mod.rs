//! Settings (`docs/ia/settings.md`): launcher preferences and the defaults
//! every game starts from. The page shows read-only values and quick
//! controls; anything typed is edited in a dialog (design language §10).
//! [`Settings`] draws the page, reads the settings and saves each one; the
//! shell owns it and passes its events on, and the application applies
//! saved preferences and runs the updater.

pub use page::{PLUGIN_GROUP, SettingsChoices, SettingsControls, SettingsInputs, TAB_GROUP, tabs};
pub use settings_actions::{Settings, SettingsEvent};
pub use text::{bytes_text, commands_text, list_text, memory_text, window_text};

mod about;
mod appearance;
mod game_defaults;
mod general;
mod java;
mod java_work;
mod look_work;
mod page;
mod plugins;
mod rows;
mod settings_actions;
mod storage;
mod storage_work;
mod text;

#[cfg(test)]
mod tests;
