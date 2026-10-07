use crate::live::SettingsView;
use crate::tr;
use lumilio_core::recommended_memory_mb;

/// `4096 MB`, or `未设置`.
#[must_use]
pub fn memory_text(value: Option<u32>) -> String {
    value.map_or_else(|| tr!("common-not-set").to_owned(), |mb| format!("{mb} MB"))
}

/// `1280 × 720 · 全屏`, or `未设置`.
#[must_use]
pub fn window_text(view: &SettingsView) -> String {
    let size = view
        .launch
        .window_width
        .zip(view.launch.window_height)
        .map(|(width, height)| format!("{width} × {height}"));
    let fullscreen = (view.launch.fullscreen == Some(true))
        .then(|| tr!("settings-window-fullscreen-value").to_owned());
    let parts: Vec<String> = size.into_iter().chain(fullscreen).collect();
    if parts.is_empty() {
        tr!("common-not-set").to_owned()
    } else {
        parts.join(" · ")
    }
}

/// `无`, or the first item with how many there are.
#[must_use]
pub fn list_text(items: &[String]) -> String {
    match items {
        [] => tr!("common-none").to_owned(),
        [only] => only.clone(),
        [first, ..] => tr!(
            "common-list-first-of",
            first = first.as_str(),
            count = items.len()
        ),
    }
}

#[must_use]
pub fn commands_text(view: &SettingsView) -> String {
    let set = [
        (
            tr!("settings-command-pre-short"),
            view.launch.pre_launch.is_some(),
        ),
        (
            tr!("settings-command-wrapper-short"),
            view.launch.wrapper.is_some(),
        ),
        (
            tr!("settings-command-post-short"),
            view.launch.post_exit.is_some(),
        ),
    ]
    .into_iter()
    .filter(|(_, on)| *on)
    .map(|(name, _)| name)
    .collect::<Vec<_>>();
    if set.is_empty() {
        tr!("common-none").to_owned()
    } else {
        set.join(tr!("common-list-separator"))
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
        Some(total) => tr!(
            "settings-memory-help",
            total = bytes_text(total * 1024 * 1024),
            recommended = recommended_memory_mb(total)
        ),
        None => tr!("settings-memory-help-unknown").to_owned(),
    }
}
