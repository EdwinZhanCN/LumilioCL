use lumilio_core::{AfterLaunch, QuickPlay};

/// `跟随默认 · value` while nothing is overridden, else the own value.
#[must_use]
pub fn followed(own: Option<String>, default: impl Into<String>) -> String {
    own.unwrap_or_else(|| format!("跟随默认 · {}", default.into()))
}

#[must_use]
pub fn window_text(width: Option<u32>, height: Option<u32>) -> Option<String> {
    width.zip(height).map(|(w, h)| format!("{w} × {h}"))
}

#[must_use]
pub fn quick_play_text(target: Option<&QuickPlay>) -> String {
    match target {
        None => "无（进入主菜单）".to_owned(),
        Some(QuickPlay::World(name)) => format!("世界 · {name}"),
        Some(QuickPlay::Server(address)) => format!("服务器 · {address}"),
    }
}

#[must_use]
pub fn after_launch_text(value: AfterLaunch) -> &'static str {
    match value {
        AfterLaunch::Keep => "保持",
        AfterLaunch::Hide => "隐藏启动器",
    }
}

/// `-` is how a command field says "this game uses none".
pub(super) const NONE_ON_PURPOSE: &str = "-";

pub(super) fn command_text(own: Option<&String>, default: Option<&String>) -> String {
    match own {
        Some(text) if text.is_empty() => "不使用".to_owned(),
        Some(text) => text.clone(),
        None => followed(None, default.map_or("无", String::as_str)),
    }
}
