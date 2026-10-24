//! What each Settings dialog asks for and how its text becomes a change.
//! Pure functions: the dialog only shows fields and calls these.

use std::path::PathBuf;

use lumilio_core::{
    DOWNLOAD_CONCURRENCY, EnvVar, LaunchTuning, MAX_MEMORY_MB, MAX_WINDOW_SIDE, MirrorRule,
    TuningError,
};

use crate::live::LiveIntent;
use crate::tr;

pub type Parsed = Result<LiveIntent, String>;

/// Each non-blank line, trimmed.
#[must_use]
pub fn lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

#[must_use]
pub fn join_lines(items: &[String]) -> String {
    items.join("\n")
}

fn number(text: &str, what: &str) -> Result<Option<u32>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    text.parse::<u32>()
        .map(Some)
        .map_err(|_| tr!("settings-number-whole", what = what))
}

/// Why core refused a tuning, in one sentence.
#[must_use]
pub fn tuning_message(error: &TuningError) -> String {
    match error {
        TuningError::WindowSize => tr!("settings-window-size", max = MAX_WINDOW_SIDE),
        TuningError::EnvironmentName(name) => {
            tr!("settings-env-name-invalid", name = name.as_str())
        }
        TuningError::ControlCharacter => tr!("settings-control-character").to_owned(),
        TuningError::Wrapper => tr!("settings-wrapper-unclosed").to_owned(),
        TuningError::QuickPlay => tr!("settings-quick-play").to_owned(),
    }
}

fn tuning(next: LaunchTuning) -> Parsed {
    let next = next.normalized();
    next.validate().map_err(|error| tuning_message(&error))?;
    Ok(LiveIntent::SetLaunchDefaults(next))
}

/// Minimum and maximum memory in MB; blank clears a limit.
pub fn memory(min: &str, max: &str) -> Parsed {
    let min_label = tr!("settings-field-min-memory");
    let max_label = tr!("settings-field-max-memory");
    let min_mb = number(min, min_label)?;
    let max_mb = number(max, max_label)?;
    for (value, what) in [(min_mb, min_label), (max_mb, max_label)] {
        if value.is_some_and(|value| value == 0 || value > MAX_MEMORY_MB) {
            return Err(tr!(
                "settings-number-range",
                what = what,
                max = MAX_MEMORY_MB
            ));
        }
    }
    if let (Some(min), Some(max)) = (min_mb, max_mb)
        && min > max
    {
        return Err(tr!("settings-memory-min-above-max").to_owned());
    }
    Ok(LiveIntent::SetMemory { min_mb, max_mb })
}

/// Window size and fullscreen: `fullscreen` is 0 off, 1 on, 2 not set.
pub fn window(current: &LaunchTuning, width: &str, height: &str, fullscreen: usize) -> Parsed {
    let mut next = current.clone();
    next.window_width = number(width, tr!("settings-field-window-width"))?;
    next.window_height = number(height, tr!("settings-field-window-height"))?;
    next.fullscreen = match fullscreen {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    };
    tuning(next)
}

pub fn jvm_arguments(current: &LaunchTuning, text: &str) -> Parsed {
    let mut next = current.clone();
    next.jvm_arguments = lines(text);
    tuning(next)
}

pub fn game_arguments(current: &LaunchTuning, text: &str) -> Parsed {
    let mut next = current.clone();
    next.game_arguments = lines(text);
    tuning(next)
}

/// `NAME=value` per line.
pub fn environment(current: &LaunchTuning, text: &str) -> Parsed {
    let mut variables = Vec::new();
    for (index, line) in lines(text).into_iter().enumerate() {
        let Some((name, value)) = line.split_once('=') else {
            return Err(tr!("settings-env-line", line = index + 1));
        };
        variables.push(EnvVar {
            name: name.trim().to_owned(),
            value: value.trim().to_owned(),
        });
    }
    let mut next = current.clone();
    next.environment = variables;
    tuning(next)
}

pub fn commands(current: &LaunchTuning, pre: &str, wrapper: &str, post: &str) -> Parsed {
    let mut next = current.clone();
    next.pre_launch = Some(pre.to_owned());
    next.wrapper = Some(wrapper.to_owned());
    next.post_exit = Some(post.to_owned());
    tuning(next)
}

/// Downloads at once; blank leaves it to the launcher.
pub fn concurrency(text: &str) -> Parsed {
    let count = number(text, tr!("settings-concurrency"))?;
    if count.is_some_and(|count| !DOWNLOAD_CONCURRENCY.contains(&count)) {
        return Err(tr!(
            "settings-concurrency-range",
            min = *DOWNLOAD_CONCURRENCY.start(),
            max = *DOWNLOAD_CONCURRENCY.end()
        ));
    }
    Ok(LiveIntent::SetConcurrency(count))
}

/// `official => mirror` per line.
pub fn mirrors(text: &str) -> Parsed {
    let mut rules = Vec::new();
    for (index, line) in lines(text).into_iter().enumerate() {
        let Some((official, mirror)) = line.split_once("=>") else {
            return Err(tr!("settings-mirror-line", line = index + 1));
        };
        let (official, mirror) = (official.trim(), mirror.trim());
        if official.is_empty() || mirror.is_empty() {
            return Err(tr!("settings-mirror-prefixes", line = index + 1));
        }
        rules.push(MirrorRule {
            official_prefix: official.to_owned(),
            mirror_prefix: mirror.to_owned(),
        });
    }
    Ok(LiveIntent::SetMirrors(rules))
}

/// One folder per line.
pub fn java_roots(text: &str) -> Parsed {
    Ok(LiveIntent::SetJavaRoots(
        lines(text).into_iter().map(PathBuf::from).collect(),
    ))
}

#[cfg(test)]
mod tests;
