use crate::tr;
use lumilio_core::{AfterLaunch, QuickPlay};

/// `跟随默认 · value` while nothing is overridden, else the own value.
#[must_use]
pub fn followed(own: Option<String>, default: impl Into<String>) -> String {
    own.unwrap_or_else(|| tr!("instance-settings-followed", value = default.into()))
}

#[must_use]
pub fn window_text(width: Option<u32>, height: Option<u32>) -> Option<String> {
    width.zip(height).map(|(w, h)| format!("{w} × {h}"))
}

#[must_use]
pub fn quick_play_text(target: Option<&QuickPlay>) -> String {
    match target {
        None => tr!("instance-settings-quick-none").to_owned(),
        Some(QuickPlay::World(name)) => tr!("instance-settings-quick-world", name = name),
        Some(QuickPlay::Server(address)) => {
            tr!("instance-settings-quick-server", address = address)
        }
    }
}

#[must_use]
pub fn after_launch_text(value: AfterLaunch) -> &'static str {
    match value {
        AfterLaunch::Keep => tr!("settings-after-launch-keep"),
        AfterLaunch::Hide => tr!("settings-after-launch-hide"),
    }
}

/// `-` is how a command field says "this game uses none".
pub(super) const NONE_ON_PURPOSE: &str = "-";

pub(super) fn command_text(own: Option<&String>, default: Option<&String>) -> String {
    match own {
        Some(text) if text.is_empty() => tr!("instance-settings-command-none").to_owned(),
        Some(text) => text.clone(),
        None => followed(None, default.map_or(tr!("common-none"), String::as_str)),
    }
}
