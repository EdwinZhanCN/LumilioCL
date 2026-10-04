use crate::live::SettingsView;
use lumilio_core::recommended_memory_mb;

/// `4096 MB`, or `未设置`.
#[must_use]
pub fn memory_text(value: Option<u32>) -> String {
    value.map_or_else(|| "未设置".to_owned(), |mb| format!("{mb} MB"))
}

/// `1280 × 720 · 全屏`, or `未设置`.
#[must_use]
pub fn window_text(view: &SettingsView) -> String {
    let size = view
        .launch
        .window_width
        .zip(view.launch.window_height)
        .map(|(width, height)| format!("{width} × {height}"));
    let fullscreen = (view.launch.fullscreen == Some(true)).then_some("全屏".to_owned());
    let parts: Vec<String> = size.into_iter().chain(fullscreen).collect();
    if parts.is_empty() {
        "未设置".to_owned()
    } else {
        parts.join(" · ")
    }
}

/// `无`, or the first item with how many there are.
#[must_use]
pub fn list_text(items: &[String]) -> String {
    match items {
        [] => "无".to_owned(),
        [only] => only.clone(),
        [first, rest @ ..] => format!("{first} 等 {} 项", rest.len() + 1),
    }
}

#[must_use]
pub fn commands_text(view: &SettingsView) -> String {
    let set = [
        ("启动前", view.launch.pre_launch.is_some()),
        ("包装", view.launch.wrapper.is_some()),
        ("退出后", view.launch.post_exit.is_some()),
    ]
    .into_iter()
    .filter(|(_, on)| *on)
    .map(|(name, _)| name)
    .collect::<Vec<_>>();
    if set.is_empty() {
        "无".to_owned()
    } else {
        set.join("、")
    }
}

/// `1.5 GB`, `320 MB`, `0 B`.
#[must_use]
pub fn bytes_text(bytes: u64) -> String {
    const KB: f64 = 1024.;
    let value = bytes as f64;
    if value >= KB * KB * KB {
        format!("{:.1} GB", value / (KB * KB * KB))
    } else if value >= KB * KB {
        format!("{:.0} MB", value / (KB * KB))
    } else if value >= KB {
        format!("{:.0} KB", value / KB)
    } else {
        format!("{bytes} B")
    }
}

pub(super) fn memory_help(total: Option<u64>) -> String {
    match total {
        Some(total) => format!(
            "本机内存 {}，推荐最大内存 {} MB。留空则交给 Java 决定；每个游戏还可以单独设置。",
            bytes_text(total * 1024 * 1024),
            recommended_memory_mb(total)
        ),
        None => "留空则交给 Java 决定；每个游戏还可以单独设置。".to_owned(),
    }
}
