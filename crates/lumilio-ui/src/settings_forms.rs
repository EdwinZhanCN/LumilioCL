//! What each Settings dialog asks for and how its text becomes a change.
//! Pure functions: the dialog only shows fields and calls these.

use std::path::PathBuf;

use lumilio_core::{
    DOWNLOAD_CONCURRENCY, EnvVar, LaunchTuning, MAX_MEMORY_MB, MAX_WINDOW_SIDE, MirrorRule,
    TuningError,
};

use crate::live::LiveIntent;

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
        .map_err(|_| format!("{what}需要填一个整数"))
}

/// Why core refused a tuning, in one sentence.
#[must_use]
pub fn tuning_message(error: &TuningError) -> String {
    match error {
        TuningError::WindowSize => {
            format!("窗口需要同时填宽和高，范围 1 到 {MAX_WINDOW_SIDE}")
        }
        TuningError::EnvironmentName(name) => {
            format!("环境变量名“{name}”不能为空，也不能含等号、空格或控制字符")
        }
        TuningError::ControlCharacter => "参数和命令里不能有控制字符".to_owned(),
        TuningError::Wrapper => "包装命令的引号没有闭合，或者没有程序名".to_owned(),
        TuningError::QuickPlay => "直接进入的目标不可用".to_owned(),
    }
}

fn tuning(next: LaunchTuning) -> Parsed {
    let next = next.normalized();
    next.validate().map_err(|error| tuning_message(&error))?;
    Ok(LiveIntent::SetLaunchDefaults(next))
}

/// Minimum and maximum memory in MB; blank clears a limit.
pub fn memory(min: &str, max: &str) -> Parsed {
    let min_mb = number(min, "最小内存")?;
    let max_mb = number(max, "最大内存")?;
    for (value, what) in [(min_mb, "最小内存"), (max_mb, "最大内存")] {
        if value.is_some_and(|value| value == 0 || value > MAX_MEMORY_MB) {
            return Err(format!("{what}要在 1 到 {MAX_MEMORY_MB} MB 之间"));
        }
    }
    if let (Some(min), Some(max)) = (min_mb, max_mb)
        && min > max
    {
        return Err("最小内存不能超过最大内存".to_owned());
    }
    Ok(LiveIntent::SetMemory { min_mb, max_mb })
}

/// Window size and fullscreen: `fullscreen` is 0 off, 1 on, 2 not set.
pub fn window(current: &LaunchTuning, width: &str, height: &str, fullscreen: usize) -> Parsed {
    let mut next = current.clone();
    next.window_width = number(width, "窗口宽度")?;
    next.window_height = number(height, "窗口高度")?;
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
            return Err(format!("第 {} 行需要写成 名称=值", index + 1));
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
    let count = number(text, "同时下载数")?;
    if count.is_some_and(|count| !DOWNLOAD_CONCURRENCY.contains(&count)) {
        return Err(format!(
            "同时下载数要在 {} 到 {} 之间",
            DOWNLOAD_CONCURRENCY.start(),
            DOWNLOAD_CONCURRENCY.end()
        ));
    }
    Ok(LiveIntent::SetConcurrency(count))
}

/// `official => mirror` per line.
pub fn mirrors(text: &str, prefer: bool) -> Parsed {
    let mut rules = Vec::new();
    for (index, line) in lines(text).into_iter().enumerate() {
        let Some((official, mirror)) = line.split_once("=>") else {
            return Err(format!("第 {} 行需要写成 官方前缀 => 镜像前缀", index + 1));
        };
        let (official, mirror) = (official.trim(), mirror.trim());
        if official.is_empty() || mirror.is_empty() {
            return Err(format!("第 {} 行的两个前缀都不能为空", index + 1));
        }
        rules.push(MirrorRule {
            official_prefix: official.to_owned(),
            mirror_prefix: mirror.to_owned(),
        });
    }
    Ok(LiveIntent::SetMirrors {
        mirrors: rules,
        prefer,
    })
}

/// One folder per line.
pub fn java_roots(text: &str) -> Parsed {
    Ok(LiveIntent::SetJavaRoots(
        lines(text).into_iter().map(PathBuf::from).collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tuned(parsed: Parsed) -> LaunchTuning {
        match parsed.unwrap() {
            LiveIntent::SetLaunchDefaults(tuning) => tuning,
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn memory_accepts_blank_ranges_and_refuses_the_rest() {
        assert_eq!(
            memory("", "4096"),
            Ok(LiveIntent::SetMemory {
                min_mb: None,
                max_mb: Some(4096)
            })
        );
        assert_eq!(
            memory(" 512 ", ""),
            Ok(LiveIntent::SetMemory {
                min_mb: Some(512),
                max_mb: None
            })
        );
        assert!(memory("abc", "").unwrap_err().contains("整数"));
        assert!(memory("0", "").unwrap_err().contains("1 到"));
        assert!(memory("", "99999999999").is_err());
        assert!(memory("8192", "4096").unwrap_err().contains("不能超过"));
    }

    #[test]
    fn a_window_needs_both_sides_and_keeps_the_other_settings() {
        let current = LaunchTuning {
            game_arguments: vec!["--demo".into()],
            ..LaunchTuning::default()
        };
        let next = tuned(window(&current, "1280", "720", 1));
        assert_eq!(
            (next.window_width, next.window_height, next.fullscreen),
            (Some(1280), Some(720), Some(true))
        );
        assert_eq!(next.game_arguments, ["--demo"]);
        let cleared = tuned(window(&next, "", "", 2));
        assert_eq!((cleared.window_width, cleared.fullscreen), (None, None));
        assert!(
            window(&current, "1280", "", 2)
                .unwrap_err()
                .contains("同时填宽和高")
        );
        assert!(window(&current, "x", "720", 2).is_err());
    }

    #[test]
    fn argument_and_environment_text_becomes_lines_and_pairs() {
        let current = LaunchTuning::default();
        assert_eq!(
            tuned(jvm_arguments(&current, " -Xss1m \n\n -Da=b ")).jvm_arguments,
            ["-Xss1m", "-Da=b"]
        );
        let env = tuned(environment(&current, "MESA_DEBUG = 1\nA=b=c"));
        assert_eq!(env.environment[0].name, "MESA_DEBUG");
        assert_eq!(env.environment[0].value, "1");
        assert_eq!(env.environment[1].value, "b=c", "only the first = splits");
        assert!(
            environment(&current, "oops")
                .unwrap_err()
                .contains("第 1 行")
        );
        assert!(
            environment(&current, "=x")
                .unwrap_err()
                .contains("环境变量名")
        );
    }

    #[test]
    fn commands_are_trimmed_and_a_bad_wrapper_is_explained() {
        let current = LaunchTuning::default();
        let next = tuned(commands(&current, " echo hi ", "", "  "));
        assert_eq!(next.pre_launch.as_deref(), Some("echo hi"));
        assert_eq!((next.wrapper, next.post_exit), (None, None));
        assert!(
            commands(&current, "", "nice \"unclosed", "")
                .unwrap_err()
                .contains("引号")
        );
    }

    #[test]
    fn concurrency_mirrors_and_folders_parse() {
        assert_eq!(concurrency(""), Ok(LiveIntent::SetConcurrency(None)));
        assert_eq!(concurrency("8"), Ok(LiveIntent::SetConcurrency(Some(8))));
        assert!(concurrency("0").is_err() && concurrency("33").is_err());
        let parsed = mirrors("https://a.example/ => https://m.example/a/\n", true).unwrap();
        assert_eq!(
            parsed,
            LiveIntent::SetMirrors {
                mirrors: vec![MirrorRule {
                    official_prefix: "https://a.example/".into(),
                    mirror_prefix: "https://m.example/a/".into()
                }],
                prefer: true
            }
        );
        assert!(mirrors("no arrow", false).unwrap_err().contains("第 1 行"));
        assert!(mirrors("a =>  ", false).unwrap_err().contains("不能为空"));
        assert_eq!(
            java_roots("/opt/a\n /opt/b ").unwrap(),
            LiveIntent::SetJavaRoots(vec!["/opt/a".into(), "/opt/b".into()])
        );
    }
}
