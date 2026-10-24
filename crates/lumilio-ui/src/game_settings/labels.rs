//! How options and their values read: in the units and words the game uses.

use lumilio_core::{GameSettingGroup, GameSettingKind, GameSettingValue, game_setting_kind};

use super::choices::{key_label, language_label};
use crate::tr;

/// The groups in the order a player meets them in the game's own menus.
pub const GROUPS: &[GameSettingGroup] = &[
    GameSettingGroup::Controls,
    GameSettingGroup::Sound,
    GameSettingGroup::Video,
    GameSettingGroup::Chat,
    GameSettingGroup::Accessibility,
    GameSettingGroup::General,
];

pub fn group_label(group: GameSettingGroup) -> &'static str {
    match group {
        GameSettingGroup::Controls => tr!("preset-group-controls"),
        GameSettingGroup::Sound => tr!("preset-group-sound"),
        GameSettingGroup::General => tr!("preset-group-general"),
        GameSettingGroup::Chat => tr!("preset-group-chat"),
        GameSettingGroup::Accessibility => tr!("preset-group-accessibility"),
        GameSettingGroup::Video => tr!("preset-group-video"),
    }
}

/// What a group holds, for the choice of what a new preset shares.
pub fn group_detail(group: GameSettingGroup) -> &'static str {
    match group {
        GameSettingGroup::Controls => tr!("preset-group-controls-detail"),
        GameSettingGroup::Sound => tr!("preset-group-sound-detail"),
        GameSettingGroup::General => tr!("preset-group-general-detail"),
        GameSettingGroup::Chat => tr!("preset-group-chat-detail"),
        GameSettingGroup::Accessibility => tr!("preset-group-accessibility-detail"),
        GameSettingGroup::Video => tr!("preset-group-video-detail"),
    }
}

pub fn setting_label(key: &str) -> String {
    let suffix = key
        .strip_prefix("key_key.")
        .map(|k| format!("key-{}", k.to_ascii_lowercase().replace('.', "-")))
        .unwrap_or_else(|| key.to_ascii_lowercase().replace('_', "-"));
    crate::i18n::lookup(&format!("preset-setting-{suffix}")).unwrap_or_else(|| key.to_owned())
}

/// The position of a percent slider as the game labels it (0.5 sensitivity
/// is 100%).
#[must_use]
pub fn shown_percent(value: &GameSettingValue, max: u16) -> Option<i64> {
    let GameSettingValue::Decimal(text) = value else {
        return None;
    };
    let fraction = text.parse::<f64>().ok()?;
    Some((fraction * f64::from(max)).round() as i64)
}

/// A choice's words, for numbered choices such as chat visibility.
pub fn choice_label(key: &str, index: i64) -> String {
    crate::i18n::lookup(&format!(
        "options-choice-{}-{index}",
        key.to_ascii_lowercase()
    ))
    .unwrap_or_else(|| index.to_string())
}

pub fn value_label(key: &str, value: &GameSettingValue) -> String {
    let kind = game_setting_kind(key);
    match (kind, value) {
        (_, GameSettingValue::Boolean(on)) => if *on {
            tr!("preset-on")
        } else {
            tr!("preset-off")
        }
        .into(),
        (Some(GameSettingKind::Percent { max }), _) => shown_percent(value, max)
            .map_or_else(|| value.to_string(), |percent| format!("{percent}%")),
        (Some(GameSettingKind::Choice { .. }), GameSettingValue::Integer(index)) => {
            choice_label(key, *index)
        }
        (Some(GameSettingKind::Number { .. }), GameSettingValue::Integer(number)) => {
            number_label(key, *number)
        }
        (Some(GameSettingKind::Key), GameSettingValue::Text(token)) => key_label(token),
        (Some(GameSettingKind::Language), GameSettingValue::Text(code)) => language_label(code),
        _ => value.to_string(),
    }
}

/// A whole number with the unit or the special word the game shows for it.
pub fn number_label(key: &str, number: i64) -> String {
    match (key, number) {
        ("fov", degrees) => format!("{degrees}°"),
        ("renderDistance" | "simulationDistance", chunks) => {
            tr!("options-chunks", count = chunks)
        }
        ("maxFps", 260) => tr!("options-unlimited").into(),
        ("maxFps", fps) => format!("{fps} FPS"),
        ("guiScale", 0) => tr!("options-auto").into(),
        ("mipmapLevels", 0) => tr!("preset-off").into(),
        (_, number) => number.to_string(),
    }
}
