//! Shared native preferences, explicit instance overrides and unconfirmed game changes.
mod catalog;
mod document;
mod edit;
mod engine;
mod storage;

#[cfg(test)]
mod tests;

pub(crate) use catalog::valid;
pub use catalog::{GameSettingGroup, GameSettingKind, game_setting_group, game_setting_kind};
pub(crate) use edit::{edit_option, options};
pub(crate) use engine::{apply, capture, resolve, view};
pub(crate) use storage::{binding, delete, list, save_binding, save_preset};
pub(crate) use storage::{new_preset, preset};

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum GameSettingValue {
    Boolean(bool),
    Integer(i64),
    /// Normalized finite decimal; strings avoid floating-point equality and JSON NaN.
    Decimal(String),
    Text(String),
}

impl GameSettingValue {
    /// A slider position from 0 to 1, normalized the way values read from the
    /// game are. Some sliders are rewritten through f32 by Minecraft;
    /// sub-micro differences are not an actionable preference.
    #[must_use]
    pub fn fraction(value: f64) -> Option<Self> {
        (value.is_finite() && (0.0..=1.0).contains(&value)).then(|| {
            let value = (value * 1_000_000.).round() / 1_000_000.;
            Self::Decimal(if value == 0. {
                "0".into()
            } else {
                value.to_string()
            })
        })
    }
}

impl std::fmt::Display for GameSettingValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Boolean(value) => value.fmt(f),
            Self::Integer(value) => value.fmt(f),
            Self::Decimal(value) | Self::Text(value) => value.fmt(f),
        }
    }
}

pub(super) type Values = BTreeMap<String, GameSettingValue>;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[non_exhaustive]
pub struct SettingsPreset {
    pub id: i64,
    pub name: String,
    pub revision: u32,
    pub values: Values,
    /// Exclusion changes sharing scope without deleting the saved preference.
    #[serde(default)]
    pub excluded: BTreeSet<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[non_exhaustive]
pub struct SettingChange {
    pub setting: String,
    pub before: GameSettingValue,
    pub value: GameSettingValue,
    /// The shared value observed when this field first diverged.
    pub shared_before: GameSettingValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingResolution {
    Instance,
    Shared,
    Restore,
    /// Explicitly confirmed after inspecting a concurrent shared change.
    ReplaceShared,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub struct GameSettingsView {
    pub instance: String,
    pub preset: Option<SettingsPreset>,
    pub overrides: Values,
    pub pending: Vec<SettingChange>,
    pub skipped: Vec<String>,
    pub recovery_required: bool,
    pub is_busy: bool,
    pub issue: Option<String>,
}

/// A launcher-side edit of one option while the game is closed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptionChange {
    /// This game only: its own options file, or an override of a shared option.
    Set(GameSettingValue),
    /// Drop this game's override so the preset's value applies again.
    Follow,
}

/// Whether a game's own options can be read and edited.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum OptionsFile {
    Ready,
    /// The game has not been started yet, so it has not written its options.
    #[default]
    Missing,
    /// The catalog does not verify this game version.
    Unsupported,
    Unreadable(String),
}

/// A game's options as the launcher edits them: what its own file holds and
/// how it relates to a preset.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub struct InstanceOptions {
    pub view: GameSettingsView,
    pub game_version: String,
    /// Supported options the game's file holds now, normalized.
    pub values: BTreeMap<String, GameSettingValue>,
    pub file: OptionsFile,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct Binding {
    pub preset: i64,
    #[serde(default)]
    pub overrides: Values,
    #[serde(default)]
    pub pending: BTreeMap<String, SettingChange>,
    pub snapshot: Option<Snapshot>,
    #[serde(default)]
    pub skipped: Vec<String>,
    pub issue: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Snapshot {
    pub game_version: String,
    pub preset_revision: u32,
    pub shared: Values,
    pub effective: Values,
    /// Durable intent precedes publication. Retained originals are recovery data.
    pub original: Option<String>,
    pub applied: String,
    pub active: bool,
    #[serde(default)]
    pub published: bool,
}

#[derive(Debug)]
pub enum GameSettingsError {
    Io(std::io::Error),
    Database(rusqlite::Error),
    Invalid(String),
    Conflict(Vec<String>),
    /// What the request was based on changed in the meantime.
    Stale,
    RecoveryRequired,
}
impl std::fmt::Display for GameSettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => e.fmt(f),
            Self::Database(e) => e.fmt(f),
            Self::Invalid(e) => f.write_str(e),
            Self::Conflict(keys) => write!(
                f,
                "shared settings changed concurrently: {}",
                keys.join(", ")
            ),
            Self::Stale => f.write_str("settings changed elsewhere; reload before editing"),
            Self::RecoveryRequired => {
                f.write_str("previous game session needs confirmation that the game has closed")
            }
        }
    }
}
impl std::error::Error for GameSettingsError {}
impl From<std::io::Error> for GameSettingsError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<rusqlite::Error> for GameSettingsError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Database(value)
    }
}

pub(crate) fn read_values(
    path: &std::path::Path,
    version: &str,
) -> Result<Values, GameSettingsError> {
    let text = document::read(path)?.ok_or_else(|| {
        GameSettingsError::Invalid("launch this game once to create options.txt".into())
    })?;
    Ok(document::Document::parse(&text)?.values(version))
}
