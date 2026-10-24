//! Game options and presets: the option table a preset and a game share, the
//! dialogs that edit options and presets outside the game, and the review of
//! changes made inside it. Domain changes go through the application, which
//! answers each dialog with its `finished`.
mod bind;
mod choices;
mod form;
mod labels;
mod option_dialog;
mod preset_dialogs;
mod review;
mod table;
#[cfg(test)]
mod tests;

pub use bind::{ChoosePreset, bind_blocked};
pub use form::Submit;
pub use labels::{group_label, setting_label, value_label};
#[cfg(test)]
use lumilio_core::{GameSettingValue, InstanceOptions, SettingsPreset};
pub use option_dialog::{Commit, OptionDialog, OptionSpec};
pub use presentation::{
    PresetIntent, attention_rows, game_status, instance_lines, option_spec, preset_lines,
};
pub use preset_dialogs::{CreatePreset, NamePreset, Naming, NewPreset, PresetScope, SourceGame};
pub use review::{ReviewActions, review};
pub use table::{OnEdit, OnToggle, OptionLine, Source, matches, sort_lines, table};

mod presentation;
