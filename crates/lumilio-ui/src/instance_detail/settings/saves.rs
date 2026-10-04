use super::super::InstanceIntent;
use super::texts::NONE_ON_PURPOSE;
use crate::settings_forms as shared;
use lumilio_core::{AfterLaunch, EnvVar, InstanceSettings, QuickPlay};
use std::path::PathBuf;

pub(super) type Saved = Result<InstanceIntent, String>;

pub(super) fn save(
    mut next: InstanceSettings,
    change: impl FnOnce(&mut InstanceSettings),
) -> Saved {
    change(&mut next);
    next.launch = next.launch.clone().normalized();
    next.launch
        .validate()
        .map_err(|error| shared::tuning_message(&error))?;
    Ok(InstanceIntent::SaveSettings(next))
}

pub(super) fn number(text: &str, what: &str) -> Result<Option<u32>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    text.parse()
        .map(Some)
        .map_err(|_| format!("{what}需要填一个整数"))
}

/// Window size (blank clears) and fullscreen (0 follow, 1 on, 2 off).
pub fn window(current: &InstanceSettings, width: &str, height: &str, fullscreen: usize) -> Saved {
    let (width, height) = (number(width, "窗口宽度")?, number(height, "窗口高度")?);
    save(current.clone(), |next| {
        next.launch.window_width = width;
        next.launch.window_height = height;
        next.launch.fullscreen = match fullscreen {
            1 => Some(true),
            2 => Some(false),
            _ => None,
        };
    })
}

/// 0 follow, 1 keep, 2 hide.
pub fn after_launch(current: &InstanceSettings, choice: usize) -> Saved {
    save(current.clone(), |next| {
        next.launch.after_launch = match choice {
            1 => Some(AfterLaunch::Keep),
            2 => Some(AfterLaunch::Hide),
            _ => None,
        };
    })
}

/// Kind 0 none, 1 a world, 2 a server.
pub fn quick_play(current: &InstanceSettings, kind: usize, target: &str) -> Saved {
    let target = target.trim();
    let chosen = match kind {
        0 => None,
        _ if target.is_empty() => return Err("请填写世界名或服务器地址".to_owned()),
        1 => Some(QuickPlay::World(target.to_owned())),
        _ => Some(QuickPlay::Server(target.to_owned())),
    };
    if let Some(problem) = chosen.as_ref().and_then(lumilio_core::quick_play_problem) {
        return Err(problem.to_owned());
    }
    save(current.clone(), |next| next.launch.quick_play = chosen)
}

pub fn java(current: &InstanceSettings, path: &str) -> Saved {
    let path = path.trim();
    save(current.clone(), |next| {
        next.java_path = (!path.is_empty()).then(|| PathBuf::from(path));
    })
}

pub fn jvm_arguments(current: &InstanceSettings, text: &str) -> Saved {
    save(current.clone(), |next| {
        next.jvm_arguments = shared::lines(text)
    })
}

/// Choice 0 follows the defaults; 1 sets its own (an empty list is "none").
pub fn game_arguments(current: &InstanceSettings, choice: usize, text: &str) -> Saved {
    let own = (choice == 1).then(|| shared::lines(text));
    save(current.clone(), |next| next.launch.game_arguments = own)
}

pub fn environment(current: &InstanceSettings, choice: usize, text: &str) -> Saved {
    let own = if choice == 1 {
        let mut variables = Vec::new();
        for (index, line) in shared::lines(text).into_iter().enumerate() {
            let Some((name, value)) = line.split_once('=') else {
                return Err(format!("第 {} 行需要写成 名称=值", index + 1));
            };
            variables.push(EnvVar {
                name: name.trim().to_owned(),
                value: value.trim().to_owned(),
            });
        }
        Some(variables)
    } else {
        None
    };
    save(current.clone(), |next| next.launch.environment = own)
}

/// Blank follows the default; `-` means this game uses none.
pub fn commands(current: &InstanceSettings, pre: &str, wrapper: &str, post: &str) -> Saved {
    let own = |text: &str| {
        let text = text.trim();
        match text {
            "" => None,
            NONE_ON_PURPOSE => Some(String::new()),
            other => Some(other.to_owned()),
        }
    };
    save(current.clone(), |next| {
        next.launch.pre_launch = own(pre);
        next.launch.wrapper = own(wrapper);
        next.launch.post_exit = own(post);
    })
}

/// Clears one group of overrides.
pub(super) fn cleared(
    current: &InstanceSettings,
    change: impl FnOnce(&mut InstanceSettings),
) -> Option<InstanceIntent> {
    save(current.clone(), change).ok()
}
