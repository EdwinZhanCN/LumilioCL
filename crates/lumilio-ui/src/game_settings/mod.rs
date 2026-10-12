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
pub use option_dialog::{Commit, OptionDialog, OptionSpec};
pub use preset_dialogs::{CreatePreset, NamePreset, Naming, NewPreset, PresetScope, SourceGame};
pub use review::{ReviewActions, review};
pub use table::{OnEdit, OnToggle, OptionLine, Source, matches, sort_lines, table};

use crate::tr;
use lumilio_core::{GameSettingValue, GameSettingsView, InstanceOptions, SettingsPreset};

/// What the Library's preset page asks the application to do.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresetIntent {
    New,
    Rename(i64),
    Copy(i64),
    /// Choose which of its options the preset shares.
    Scope(i64),
    /// Delete it; the page has asked already.
    Delete(i64),
    /// Open the editor for one of its options.
    Edit {
        preset: i64,
        key: String,
    },
    /// Switch one of its options at once, from the value the page showed.
    Set {
        preset: i64,
        key: String,
        before: GameSettingValue,
        value: GameSettingValue,
    },
}

/// The options a preset shares, in the game's order.
#[must_use]
pub fn preset_lines(preset: &SettingsPreset) -> Vec<OptionLine> {
    let mut lines: Vec<OptionLine> = preset
        .values
        .iter()
        .filter(|(key, _)| !preset.excluded.contains(*key))
        .map(|(key, value)| OptionLine {
            key: key.clone(),
            value: value.clone(),
            source: Source::Own,
        })
        .collect();
    sort_lines(&mut lines);
    lines
}

/// The value a game's preset gives an option, if it shares it.
fn shared_value<'a>(view: &'a GameSettingsView, key: &str) -> Option<&'a GameSettingValue> {
    view.preset
        .as_ref()
        .filter(|preset| !preset.excluded.contains(key))
        .and_then(|preset| preset.values.get(key))
}

/// A game's options as its next launch will have them, saying where each
/// comes from.
#[must_use]
pub fn instance_lines(options: &InstanceOptions) -> Vec<OptionLine> {
    let view = &options.view;
    let mut lines: Vec<OptionLine> = options
        .values
        .iter()
        .map(|(key, own)| {
            let pending = view.pending.iter().find(|change| change.setting == *key);
            let (value, source) = match (shared_value(view, key), pending, view.overrides.get(key))
            {
                (Some(_), Some(change), _) => (change.value.clone(), Source::Pending),
                (Some(_), None, Some(kept)) => (kept.clone(), Source::Override),
                (Some(shared), None, None) => (shared.clone(), Source::Following),
                (None, ..) => (own.clone(), Source::Own),
            };
            OptionLine {
                key: key.clone(),
                value,
                source,
            }
        })
        .collect();
    sort_lines(&mut lines);
    lines
}

/// What the editor of one of a game's options starts from.
#[must_use]
pub fn option_spec(options: &InstanceOptions, key: &str) -> Option<OptionSpec> {
    let line = instance_lines(options)
        .into_iter()
        .find(|line| line.key == key)?;
    let follow = shared_value(&options.view, key).cloned();
    let note = options
        .view
        .preset
        .as_ref()
        .filter(|_| follow.is_some())
        .map(|preset| tr!("options-note-shared", preset = preset.name.as_str()));
    Some(OptionSpec {
        key: key.to_owned(),
        value: line.value,
        follow,
        following: line.source == Source::Following,
        note,
    })
}

/// How a game that uses a preset stands, in a few words, and whether it
/// needs the player.
#[must_use]
pub fn game_status(view: &GameSettingsView) -> (String, bool) {
    if view.is_busy {
        (tr!("options-status-busy").into(), false)
    } else if view.recovery_required {
        (tr!("options-status-recovery").into(), true)
    } else if view.issue.is_some() {
        (tr!("options-status-error").into(), true)
    } else if !view.pending.is_empty() {
        (
            tr!("options-status-pending", count = view.pending.len()),
            true,
        )
    } else if !view.overrides.is_empty() {
        (
            tr!("options-status-overrides", count = view.overrides.len()),
            false,
        )
    } else {
        (tr!("options-status-following").into(), false)
    }
}

/// Home's attention rows for games whose options need the player.
pub fn attention_rows(
    views: &[GameSettingsView],
    games: &[crate::live::LibraryCard],
) -> Vec<crate::home::AttentionRow> {
    views
        .iter()
        .filter_map(|view| {
            let game = games.iter().find(|g| g.id == view.instance)?;
            let (title, detail) = if view.recovery_required {
                (
                    tr!("preset-attention-recovery"),
                    tr!("options-recovery-help").to_owned(),
                )
            } else if view.issue.is_some() {
                (
                    tr!("preset-attention-error"),
                    tr!("preset-attention-error-help").to_owned(),
                )
            } else if !view.pending.is_empty() {
                (
                    tr!("preset-attention"),
                    tr!("preset-attention-help", count = view.pending.len()),
                )
            } else {
                return None;
            };
            Some(crate::home::AttentionRow {
                instance: view.instance.clone(),
                name: game.name.clone(),
                title: title.into(),
                detail,
                // ia[home]: 处理游戏选项 | 需要留意里的游戏选项行「处理…」 | 打开该游戏的 设置 › 选项，待决定的修改在最上面
                action: Some((
                    crate::instance_detail::ProblemAction::Settings(
                        crate::instance_detail::SETTINGS_OPTIONS,
                    ),
                    tr!("preset-attention-review"),
                )),
                more: 0,
            })
        })
        .collect()
}
