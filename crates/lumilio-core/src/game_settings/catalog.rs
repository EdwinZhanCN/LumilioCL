//! Verified stable vanilla preference encodings. Unsupported releases are never guessed.
//! Encoding/ranges cross-checked against Modrinth App's game_options/catalog/{controls,sound,
//! accessibility,chat,video,version_changes}.rs at commit 71f35eb8 (GPL-3.0-only).
//! This catalog is intentionally narrower: no graphics-mode or legacy numeric-key conversion.

use super::GameSettingValue;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameSettingGroup {
    Controls,
    Sound,
    Video,
    Chat,
    Accessibility,
    General,
}

/// How an option is edited, in the units the game shows. Ranges are the game's own.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameSettingKind {
    Toggle,
    /// A 0–1 slider the game labels as 0–`max` percent.
    Percent {
        max: u16,
    },
    /// Whole numbers in an inclusive range; the field of view is in degrees.
    Number {
        min: i64,
        max: i64,
    },
    /// One of `count` numbered choices, starting at 0.
    Choice {
        count: u8,
    },
    /// A keyboard key or mouse button token such as `key.keyboard.w`.
    Key,
    /// A language code such as `en_us`.
    Language,
}

pub fn game_setting_group(key: &str) -> GameSettingGroup {
    if key.starts_with("key_")
        || matches!(
            key,
            "mouseSensitivity"
                | "invertYMouse"
                | "autoJump"
                | "rawMouseInput"
                | "toggleCrouch"
                | "toggleSprint"
        )
    {
        GameSettingGroup::Controls
    } else if key.starts_with("soundCategory_") {
        GameSettingGroup::Sound
    } else if matches!(
        key,
        "fov"
            | "gamma"
            | "renderDistance"
            | "simulationDistance"
            | "maxFps"
            | "guiScale"
            | "entityShadows"
            | "bobView"
            | "mipmapLevels"
    ) {
        GameSettingGroup::Video
    } else if key.starts_with("chat") {
        GameSettingGroup::Chat
    } else if matches!(
        key,
        "subtitles"
            | "highContrast"
            | "darknessEffectScale"
            | "damageTiltStrength"
            | "screenEffectScale"
            | "fovEffectScale"
    ) {
        GameSettingGroup::Accessibility
    } else {
        GameSettingGroup::General
    }
}

/// How a supported option is edited, whatever release holds it.
pub fn game_setting_kind(key: &str) -> Option<GameSettingKind> {
    spec(key).map(|(kind, _)| kind)
}

type Release = (u32, u32, u32);

/// The newest release the catalog verifies. A value checked against it is
/// valid wherever the option exists; older releases only lack options.
const NEWEST: &str = "26.3";

// Exact vanilla bindings only; mod bindings remain local even with the key_ prefix.
const BINDINGS: &[&str] = &[
    "forward",
    "left",
    "back",
    "right",
    "jump",
    "sneak",
    "sprint",
    "inventory",
    "swapOffhand",
    "drop",
    "use",
    "attack",
    "pickItem",
    "chat",
    "playerlist",
    "command",
    "screenshot",
    "togglePerspective",
    "smoothCamera",
    "fullscreen",
    "spectatorOutlines",
    "advancements",
    "saveToolbarActivator",
    "loadToolbarActivator",
    "hotbar.1",
    "hotbar.2",
    "hotbar.3",
    "hotbar.4",
    "hotbar.5",
    "hotbar.6",
    "hotbar.7",
    "hotbar.8",
    "hotbar.9",
];

const SOUND: &[&str] = &[
    "master", "music", "record", "weather", "block", "hostile", "neutral", "player", "ambient",
    "voice",
];

/// An option's kind and the first release that writes it.
fn spec(key: &str) -> Option<(GameSettingKind, Release)> {
    use GameSettingKind::{Choice, Key, Language, Number, Percent, Toggle};
    const ALWAYS: Release = (1, 16, 0);
    const PERCENT: GameSettingKind = Percent { max: 100 };
    if let Some(binding) = key.strip_prefix("key_key.") {
        return BINDINGS.contains(&binding).then_some((Key, ALWAYS));
    }
    if let Some(category) = key.strip_prefix("soundCategory_") {
        return match category {
            "ui" => Some((PERCENT, (1, 21, 6))),
            category if SOUND.contains(&category) => Some((PERCENT, ALWAYS)),
            _ => None,
        };
    }
    Some(match key {
        "lang" => (Language, ALWAYS),
        "invertYMouse" | "autoJump" | "rawMouseInput" | "toggleCrouch" | "toggleSprint"
        | "entityShadows" | "bobView" | "subtitles" | "chatColors" | "chatLinks"
        | "chatLinksPrompt" => (Toggle, ALWAYS),
        "highContrast" => (Toggle, (1, 19, 4)),
        "renderDistance" => (Number { min: 2, max: 32 }, ALWAYS),
        "simulationDistance" => (Number { min: 5, max: 32 }, (1, 18, 0)),
        "maxFps" => (Number { min: 10, max: 260 }, ALWAYS),
        "guiScale" => (Number { min: 0, max: 16 }, ALWAYS),
        "mipmapLevels" => (Number { min: 0, max: 4 }, ALWAYS),
        "fov" => (Number { min: 30, max: 110 }, ALWAYS),
        "chatVisibility" => (Choice { count: 3 }, ALWAYS),
        // The game labels sensitivity as twice its stored fraction (0.5 is 100%).
        "mouseSensitivity" => (Percent { max: 200 }, ALWAYS),
        "gamma"
        | "chatOpacity"
        | "chatScale"
        | "chatWidth"
        | "chatHeightFocused"
        | "chatHeightUnfocused"
        | "chatBackgroundOpacity"
        | "chatLineSpacing" => (PERCENT, ALWAYS),
        "screenEffectScale" | "fovEffectScale" => (PERCENT, (1, 16, 2)),
        "darknessEffectScale" => (PERCENT, (1, 19, 0)),
        "damageTiltStrength" => (PERCENT, (1, 19, 4)),
        _ => return None,
    })
}

fn release(version: &str) -> Option<Release> {
    let parts: Vec<_> = version.split('.').collect();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    Some((
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts.get(2).map_or(Some(0), |p| p.parse().ok())?,
    ))
}

pub(super) fn supported(version: &str) -> bool {
    release(version).is_some_and(|v| {
        ((1, 16, 0)..=(1, 21, 11)).contains(&v) || ((26, 1, 0)..=(26, 3, 0)).contains(&v)
    })
}

pub(super) fn decode(key: &str, raw: &str, version: &str) -> Option<GameSettingValue> {
    let v = release(version)?;
    if !supported(version) {
        return None;
    }
    let (kind, since) = spec(key)?;
    if v < since {
        return None;
    }
    let raw = raw.trim();
    match kind {
        GameSettingKind::Key => {
            let text = raw
                .strip_prefix('"')
                .and_then(|_| serde_json::from_str::<String>(raw).ok())
                .unwrap_or_else(|| raw.into());
            (text.starts_with("key.keyboard.") || text.starts_with("key.mouse."))
                .then_some(GameSettingValue::Text(text))
        }
        GameSettingKind::Language => {
            let text = raw.trim_matches('"');
            (!text.is_empty()
                && text.len() <= 32
                && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
            .then(|| GameSettingValue::Text(text.into()))
        }
        GameSettingKind::Toggle => raw.parse().ok().map(GameSettingValue::Boolean),
        GameSettingKind::Number { .. } if key == "fov" => {
            let value = raw.parse::<f64>().ok()?;
            let degrees = if (30.0..=110.0).contains(&value) {
                value
            } else if (-1.0..=1.0).contains(&value) {
                value * 40.0 + 70.0
            } else {
                return None;
            };
            (degrees.is_finite() && (degrees - degrees.round()).abs() <= 0.000001)
                .then(|| GameSettingValue::Integer(degrees.round() as i64))
        }
        GameSettingKind::Number { min, max } => {
            let value = raw.parse::<i64>().ok()?;
            (min..=max)
                .contains(&value)
                .then_some(GameSettingValue::Integer(value))
        }
        GameSettingKind::Choice { count } => {
            let value = raw.parse::<i64>().ok()?;
            (0..i64::from(count))
                .contains(&value)
                .then_some(GameSettingValue::Integer(value))
        }
        GameSettingKind::Percent { .. } => GameSettingValue::fraction(raw.parse::<f64>().ok()?),
    }
}

pub(super) fn encode(key: &str, value: &GameSettingValue, version: &str) -> Option<String> {
    let raw = if key == "fov" {
        let GameSettingValue::Integer(degrees) = value else {
            return None;
        };
        ((*degrees as f64 - 70.0) / 40.0).to_string()
    } else {
        value.to_string()
    };
    if decode(key, &raw, version).as_ref() != Some(value) {
        return None;
    }
    Some(raw)
}

/// Whether `value` is a normalized value of a known option. Preset values are
/// version independent, so they are checked against the newest release.
pub(crate) fn valid(key: &str, value: &GameSettingValue) -> bool {
    encode(key, value, NEWEST).is_some()
}
