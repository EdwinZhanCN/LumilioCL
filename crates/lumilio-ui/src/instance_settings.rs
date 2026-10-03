//! The Settings tab of an Instance (`docs/ia/instance/settings.md`): five
//! groups of rows. Each shows the value in effect and where it comes from;
//! anything typed is edited in a dialog, and "恢复默认" removes the override
//! instead of copying the launcher default into the instance.
//!
//! A child module of `instance_detail`, so it shares the view's private state.

use std::path::PathBuf;
use std::rc::Rc;

use gpui::{AnyElement, Context, IntoElement, SharedString, prelude::*};
use gpui_component::{h_flex, v_flex};
use lumilio_core::{
    AfterLaunch, EnvVar, InstanceSettings, LauncherSettings, QuickPlay, recommended_memory_mb,
};

use super::{InstanceDetailView, InstanceIntent};
use crate::pages::settings::{bytes_text, list_text};
use crate::settings_dialog::{DialogSpec, FieldKind, FieldSpec, SettingsDialog};
use crate::settings_forms as shared;
use crate::theme::ShellColors;
use crate::{kit, live};

pub const SUBTABS: [&str; 5] = ["游戏", "运行时", "Java", "性能", "高级"];

const FULLSCREEN: [&str; 3] = ["跟随默认", "开", "关"];
const AFTER_LAUNCH: [&str; 3] = ["跟随默认", "保持", "隐藏启动器"];
const QUICK_PLAY: [&str; 3] = ["无", "世界", "服务器"];
const FOLLOW_OR_OWN: [&str; 2] = ["跟随默认", "自己设置"];

// ── what a row says ─────────────────────────────────────────────────────

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
const NONE_ON_PURPOSE: &str = "-";

fn command_text(own: Option<&String>, default: Option<&String>) -> String {
    match own {
        Some(text) if text.is_empty() => "不使用".to_owned(),
        Some(text) => text.clone(),
        None => followed(None, default.map_or("无", String::as_str)),
    }
}

// ── forms: text in, a whole new `InstanceSettings` out ──────────────────

type Saved = Result<InstanceIntent, String>;

fn save(mut next: InstanceSettings, change: impl FnOnce(&mut InstanceSettings)) -> Saved {
    change(&mut next);
    next.launch = next.launch.clone().normalized();
    next.launch
        .validate()
        .map_err(|error| shared::tuning_message(&error))?;
    Ok(InstanceIntent::SaveSettings(next))
}

fn number(text: &str, what: &str) -> Result<Option<u32>, String> {
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
fn cleared(
    current: &InstanceSettings,
    change: impl FnOnce(&mut InstanceSettings),
) -> Option<InstanceIntent> {
    save(current.clone(), change).ok()
}

// ── the tab ─────────────────────────────────────────────────────────────

impl InstanceDetailView {
    fn setting_row(
        &self,
        id: &'static str,
        label: &'static str,
        help: Option<String>,
        value: impl Into<SharedString>,
        control: Option<AnyElement>,
        colors: ShellColors,
    ) -> gpui::Stateful<gpui::Div> {
        kit::value_row(id, label, help.map(Into::into), value, control, colors)
            .debug_selector(move || id.to_owned())
    }

    fn edit_spec(
        &self,
        id: &'static str,
        spec: impl Fn() -> DialogSpec<InstanceIntent> + 'static,
    ) -> AnyElement {
        let handler = self.handler.clone();
        let busy = self.busy;
        crate::theme::clickable(
            kit::ghost(id, "编辑", move |window, cx| {
                SettingsDialog::open(spec(), handler.clone(), window, cx);
            })
            .disabled(busy)
            .debug_selector(move || id.to_owned()),
            !busy,
        )
        .into_any_element()
    }

    /// The Settings tab: a second-level choice and that group's rows.
    pub(super) fn settings_tab(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        let sub = self.settings_sub.min(SUBTABS.len() - 1);
        let entity = cx.entity().downgrade();
        let segments = kit::segments(
            "instance-settings-sub",
            &SUBTABS,
            sub,
            move |index, _, cx| {
                let _ = entity.update(cx, |view, cx| {
                    view.settings_sub = index;
                    cx.notify();
                });
            },
        );
        let body = match sub {
            0 => self.group_game(colors),
            1 => self.group_runtime(colors, cx),
            2 => self.group_java(colors),
            3 => self.performance(colors, cx),
            _ => self.group_advanced(colors),
        };
        v_flex()
            .w_full()
            .gap_5()
            .child(h_flex().child(segments))
            .child(kit::entrance(body, ("instance-settings-body", sub)))
            .into_any_element()
    }

    fn group_game(&self, colors: ShellColors) -> AnyElement {
        let record = self.record.as_ref().expect("loaded");
        let own = record.settings.clone();
        let defaults: &LauncherSettings = &self.defaults;

        let size_default = window_text(defaults.launch.window_width, defaults.launch.window_height)
            .unwrap_or_else(|| "未设置".to_owned());
        let overridden = own.launch.window_width.is_some()
            || own.launch.window_height.is_some()
            || own.launch.fullscreen.is_some();
        let size =
            window_text(own.launch.window_width, own.launch.window_height).unwrap_or(size_default);
        let fullscreen = own
            .launch
            .fullscreen
            .or(defaults.launch.fullscreen)
            .unwrap_or(false);
        let effective = format!("{size}{}", if fullscreen { " · 全屏" } else { "" });
        let window_value = if overridden {
            format!("{effective}（已自定义）")
        } else {
            followed(None, effective)
        };
        let window_row = {
            let current = own.clone();
            // ia[instance.settings]: 窗口大小、全屏 | 设置 · 游戏组，值 + [编辑] 弹窗 | 宽高一起填（留空跟随默认）；全屏 关 / 开 / 跟随默认；恢复默认＝移除覆盖 | L-SET-01
            self.setting_row(
                "isettings-window",
                "窗口大小与全屏",
                Some("宽和高需要一起填；留空跟随启动器的默认。".to_owned()),
                window_value,
                Some(self.edit_spec("isettings-window-edit", move || DialogSpec {
                    title: "窗口大小与全屏",
                    intro: Some("只改这个游戏；留空或选“跟随默认”就用启动器的默认值。"),
                    fields: vec![
                            FieldSpec {
                                label: "宽度",
                                help: None,
                                placeholder: "跟随默认",
                                value: current
                                    .launch
                                    .window_width
                                    .map(|v| v.to_string())
                                    .unwrap_or_default(),
                                kind: FieldKind::Line,
                            },
                            FieldSpec {
                                label: "高度",
                                help: None,
                                placeholder: "跟随默认",
                                value: current
                                    .launch
                                    .window_height
                                    .map(|v| v.to_string())
                                    .unwrap_or_default(),
                                kind: FieldKind::Line,
                            },
                            FieldSpec {
                                label: "全屏启动",
                                help: None,
                                placeholder: "",
                                value: String::new(),
                                kind: FieldKind::Choice {
                                    labels: &FULLSCREEN,
                                    selected: match current.launch.fullscreen {
                                        None => 0,
                                        Some(true) => 1,
                                        Some(false) => 2,
                                    },
                                },
                            },
                        ],
                    parse: {
                        let current = current.clone();
                        Rc::new(move |values| {
                            window(
                                &current,
                                &values[0],
                                &values[1],
                                values[2].parse().unwrap_or(0),
                            )
                        })
                    },
                    reset: cleared(&current, |next| {
                        next.launch.window_width = None;
                        next.launch.window_height = None;
                        next.launch.fullscreen = None;
                    }),
                })),
                colors,
            )
        };

        let after_row = {
            let current = own.clone();
            // ia[instance.settings]: 进入游戏后 | 设置 · 游戏组，值 + [编辑] 弹窗 | 保持 / 隐藏启动器 / 跟随默认 | L-SET-01
            self.setting_row(
                "isettings-after",
                "进入游戏后",
                Some("这个游戏开始运行后，启动器窗口怎么处理。".to_owned()),
                followed(
                    own.launch
                        .after_launch
                        .map(|v| after_launch_text(v).to_owned()),
                    after_launch_text(defaults.preferences.after_launch),
                ),
                Some(self.edit_spec("isettings-after-edit", move || DialogSpec {
                    title: "进入游戏后",
                    intro: None,
                    fields: vec![FieldSpec {
                        label: "启动器窗口",
                        help: None,
                        placeholder: "",
                        value: String::new(),
                        kind: FieldKind::Choice {
                            labels: &AFTER_LAUNCH,
                            selected: match current.launch.after_launch {
                                None => 0,
                                Some(AfterLaunch::Keep) => 1,
                                Some(AfterLaunch::Hide) => 2,
                            },
                        },
                    }],
                    parse: {
                        let current = current.clone();
                        Rc::new(move |values| {
                            after_launch(&current, values[0].parse().unwrap_or(0))
                        })
                    },
                    reset: cleared(&current, |next| next.launch.after_launch = None),
                })),
                colors,
            )
        };

        let quick_row = {
            let current = own.clone();
            // ia[instance.settings]: 直接进入（Quick Play） | 设置 · 游戏组，值 + [编辑] 弹窗：类型 + 目标 | 启动后直达世界或服务器；版本不支持单人世界时启动会说明原因；世界页的「进入」用同一个能力 | H-PLAY-07
            self.setting_row(
                "isettings-quick",
                "直接进入",
                Some(
                    "开始游戏后直接进入一个世界或服务器。较旧的游戏版本不能直接进入单人世界。"
                        .to_owned(),
                ),
                quick_play_text(own.launch.quick_play.as_ref()),
                Some(self.edit_spec("isettings-quick-edit", move || {
                    let (kind, target) = match &current.launch.quick_play {
                        None => (0, String::new()),
                        Some(QuickPlay::World(name)) => (1, name.clone()),
                        Some(QuickPlay::Server(address)) => (2, address.clone()),
                    };
                    DialogSpec {
                        title: "直接进入",
                        intro: Some("世界名是存档文件夹的名字；服务器写成 地址 或 地址:端口。"),
                        fields: vec![
                            FieldSpec {
                                label: "类型",
                                help: None,
                                placeholder: "",
                                value: String::new(),
                                kind: FieldKind::Choice {
                                    labels: &QUICK_PLAY,
                                    selected: kind,
                                },
                            },
                            FieldSpec {
                                label: "目标",
                                help: None,
                                placeholder: "世界名或服务器地址",
                                value: target,
                                kind: FieldKind::Line,
                            },
                        ],
                        parse: {
                            let current = current.clone();
                            Rc::new(move |values| {
                                quick_play(&current, values[0].parse().unwrap_or(0), &values[1])
                            })
                        },
                        reset: cleared(&current, |next| next.launch.quick_play = None),
                    }
                })),
                colors,
            )
        };
        kit::list(vec![window_row, after_row, quick_row], colors).into_any_element()
    }

    fn group_runtime(&self, colors: ShellColors, _cx: &mut Context<Self>) -> AnyElement {
        let record = self.record.as_ref().expect("loaded");
        let busy = self.busy;
        let handler = self.handler.clone();
        let button =
            |id: &'static str, label: &'static str, intent: InstanceIntent| -> AnyElement {
                let handler = handler.clone();
                crate::theme::clickable(
                    kit::ghost(id, label, move |window, cx| {
                        handler(intent.clone(), window, cx)
                    })
                    .disabled(busy)
                    .debug_selector(move || id.to_owned()),
                    !busy,
                )
                .into_any_element()
            };
        let rows = vec![
            // ia[instance.settings]: 更换游戏版本 / 加载器 | 设置 · 运行时，「游戏版本」「加载器」行的 [更换…] → 弹窗（加载器分段 + 版本选择器，起点是游戏现在的组合） | 警告“装好的 Mod 可能不兼容，先建快照” + [先建快照]；与现在相同时不能提交；失败回到旧组合 | L-LIB-07
            self.setting_row(
                "isettings-version",
                "游戏版本",
                Some("更换前建议先建一个快照；世界和设置不会被改动。".to_owned()),
                record.game_version.clone(),
                Some(button(
                    "isettings-version-change",
                    "更换…",
                    InstanceIntent::OpenRuntimeChange,
                )),
                colors,
            ),
            self.setting_row(
                "isettings-loader",
                "加载器",
                Some("已装好的 Mod 可能和新的加载器不兼容。".to_owned()),
                match &record.loader_version {
                    Some(version) => format!("{} {version}", live::loader_label(record.loader)),
                    None => live::loader_label(record.loader).to_owned(),
                },
                Some(button(
                    "isettings-loader-change",
                    "更换…",
                    InstanceIntent::OpenRuntimeChange,
                )),
                colors,
            ),
            // ia[instance.settings]: 修复游戏文件 | 设置 · 运行时，「游戏文件」行的 [修复] | 逐个核对游戏文件，缺的或损坏的重新下载；世界、Mod 和设置不会被改动；后台任务 | H-INSTANCE-11
            self.setting_row(
                "isettings-files",
                "游戏文件",
                Some(
                    "逐个核对游戏文件，缺的或损坏的会重新下载；世界、Mod 和设置不会被改动。"
                        .to_owned(),
                ),
                if record.installed {
                    "已安装"
                } else {
                    "未下载"
                },
                Some(button("isettings-repair", "修复", InstanceIntent::Repair)),
                colors,
            ),
        ];
        kit::list(rows, colors).into_any_element()
    }

    fn group_java(&self, colors: ShellColors) -> AnyElement {
        let record = self.record.as_ref().expect("loaded");
        let own = record.settings.clone();
        let java_row = {
            let current = own.clone();
            // ia[instance.settings]: 指定 Java | 设置 · Java 组，值 + [编辑] 弹窗 | 路径留空自动选择；保存时检查路径存在；已发现的 Java 列表在全局设置·Java | L-RUN-01
            self.setting_row(
                "isettings-java",
                "Java",
                Some("自动选择会按这个游戏需要的 Java 版本挑一个已发现的；也可以指定一个。".to_owned()),
                own.java_path
                    .as_ref()
                    .map_or_else(|| "自动选择".to_owned(), |path| path.display().to_string()),
                Some(self.edit_spec("isettings-java-edit", move || DialogSpec {
                    title: "Java",
                    intro: Some("留空表示自动选择。指定时填 java 程序或 JDK 文件夹的路径；已发现的 Java 在“设置 › Java”里。"),
                    fields: vec![FieldSpec {
                        label: "路径",
                        help: None,
                        placeholder: "自动选择",
                        value: current
                            .java_path
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_default(),
                        kind: FieldKind::Path {
                            files: true,
                            directories: true,
                            prompt: "选择 java 程序或 JDK 文件夹",
                        },
                    }],
                    parse: {
                        let current = current.clone();
                        Rc::new(move |values| java(&current, &values[0]))
                    },
                    reset: cleared(&current, |next| next.java_path = None),
                })),
                colors,
            )
        };
        let jvm_row = {
            let current = own.clone();
            let default = list_text(&self.defaults.launch.jvm_arguments);
            // ia[instance.settings]: Java 参数 | 设置 · Java 组，值 + [编辑] 弹窗 | 多行文本，每行一个参数 | L-SET-01
            self.setting_row(
                "isettings-jvm",
                "Java 参数",
                Some("设置后替换启动器默认的 Java 参数；留空则跟随默认。".to_owned()),
                if own.jvm_arguments.is_empty() {
                    followed(None, default)
                } else {
                    list_text(&own.jvm_arguments)
                },
                Some(self.edit_spec("isettings-jvm-edit", move || DialogSpec {
                    title: "Java 参数",
                    intro: Some("每行一个参数。留空则跟随启动器默认。"),
                    fields: vec![FieldSpec {
                        label: "参数",
                        help: None,
                        placeholder: "-XX:+UseG1GC",
                        value: shared::join_lines(&current.jvm_arguments),
                        kind: FieldKind::Lines { rows: 4 },
                    }],
                    parse: {
                        let current = current.clone();
                        Rc::new(move |values| jvm_arguments(&current, &values[0]))
                    },
                    reset: cleared(&current, |next| next.jvm_arguments.clear()),
                })),
                colors,
            )
        };
        kit::list(vec![java_row, jvm_row], colors).into_any_element()
    }

    pub(super) fn machine_memory_row(
        &self,
        colors: ShellColors,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        let total = self.machine_memory_mb?;
        Some(self.setting_row(
            "isettings-machine-memory",
            "本机内存",
            Some("推荐的最大内存是本机内存的一半，不超过 8 GB。".to_owned()),
            format!(
                "{} · 推荐最大 {} MB",
                bytes_text(total * 1024 * 1024),
                recommended_memory_mb(total)
            ),
            None,
            colors,
        ))
    }

    fn group_advanced(&self, colors: ShellColors) -> AnyElement {
        let record = self.record.as_ref().expect("loaded");
        let own = record.settings.clone();
        let defaults = &self.defaults.launch;

        let args_row = {
            let current = own.clone();
            let default = list_text(&defaults.game_arguments);
            // ia[instance.settings]: 游戏参数 | 设置 · 高级组，值 + [编辑] 弹窗 | 来源（跟随默认 / 自己设置）+ 多行 | L-SET-01
            self.setting_row(
                "isettings-game-args",
                "游戏参数",
                Some(
                    "传给游戏本身的参数。自己设置后替换默认；设置为空表示这个游戏不用任何参数。"
                        .to_owned(),
                ),
                match &own.launch.game_arguments {
                    None => followed(None, default),
                    Some(list) if list.is_empty() => "无".to_owned(),
                    Some(list) => list_text(list),
                },
                Some(
                    self.edit_spec("isettings-game-args-edit", move || DialogSpec {
                        title: "游戏参数",
                        intro: Some("每行一个参数。"),
                        fields: vec![
                            FieldSpec {
                                label: "来源",
                                help: None,
                                placeholder: "",
                                value: String::new(),
                                kind: FieldKind::Choice {
                                    labels: &FOLLOW_OR_OWN,
                                    selected: usize::from(current.launch.game_arguments.is_some()),
                                },
                            },
                            FieldSpec {
                                label: "参数",
                                help: None,
                                placeholder: "--demo",
                                value: shared::join_lines(
                                    current.launch.game_arguments.as_deref().unwrap_or_default(),
                                ),
                                kind: FieldKind::Lines { rows: 4 },
                            },
                        ],
                        parse: {
                            let current = current.clone();
                            Rc::new(move |values| {
                                game_arguments(&current, values[0].parse().unwrap_or(0), &values[1])
                            })
                        },
                        reset: cleared(&current, |next| next.launch.game_arguments = None),
                    }),
                ),
                colors,
            )
        };

        let env_lines = |variables: &[EnvVar]| -> Vec<String> {
            variables
                .iter()
                .map(|variable| format!("{}={}", variable.name, variable.value))
                .collect()
        };
        let env_row = {
            let current = own.clone();
            let default = list_text(&env_lines(&defaults.environment));
            // ia[instance.settings]: 环境变量 | 设置 · 高级组，值 + [编辑] 弹窗 | 来源 + 每行 名称=值 | L-SET-01
            self.setting_row(
                "isettings-env",
                "环境变量",
                Some("游戏进程额外带上的环境变量。自己设置后替换默认。".to_owned()),
                match &own.launch.environment {
                    None => followed(None, default),
                    Some(list) if list.is_empty() => "无".to_owned(),
                    Some(list) => list_text(&env_lines(list)),
                },
                Some(self.edit_spec("isettings-env-edit", move || DialogSpec {
                    title: "环境变量",
                    intro: Some("每行一个，写成 名称=值。"),
                    fields: vec![
                        FieldSpec {
                            label: "来源",
                            help: None,
                            placeholder: "",
                            value: String::new(),
                            kind: FieldKind::Choice {
                                labels: &FOLLOW_OR_OWN,
                                selected: usize::from(current.launch.environment.is_some()),
                            },
                        },
                        FieldSpec {
                            label: "变量",
                            help: None,
                            placeholder: "MESA_DEBUG=1",
                            value: current
                                .launch
                                .environment
                                .as_deref()
                                .map(|list| env_lines(list).join("\n"))
                                .unwrap_or_default(),
                            kind: FieldKind::Lines { rows: 4 },
                        },
                    ],
                    parse: {
                        let current = current.clone();
                        Rc::new(move |values| {
                            environment(&current, values[0].parse().unwrap_or(0), &values[1])
                        })
                    },
                    reset: cleared(&current, |next| next.launch.environment = None),
                })),
                colors,
            )
        };

        let commands_row = {
            let current = own.clone();
            let value = format!(
                "启动前：{} · 包装：{} · 退出后：{}",
                command_text(own.launch.pre_launch.as_ref(), defaults.pre_launch.as_ref()),
                command_text(own.launch.wrapper.as_ref(), defaults.wrapper.as_ref()),
                command_text(own.launch.post_exit.as_ref(), defaults.post_exit.as_ref()),
            );
            // ia[instance.settings]: 启动前 / 包装 / 退出后命令 | 设置 · 高级组，值 + [编辑] 弹窗 | 三个输入：留空跟随默认，填 - 表示这个游戏不使用；说明可用变量 | L-SET-01
            self.setting_row(
                "isettings-commands",
                "启动前、包装与退出后命令",
                Some("命令由你自己填写，会以当前用户的权限运行。".to_owned()),
                value,
                Some(self.edit_spec("isettings-commands-edit", move || {
                    let shown = |own: &Option<String>| match own {
                        None => String::new(),
                        Some(text) if text.is_empty() => NONE_ON_PURPOSE.to_owned(),
                        Some(text) => text.clone(),
                    };
                    DialogSpec {
                        title: "命令",
                        intro: Some("留空跟随启动器默认；填 - 表示这个游戏不使用。命令以你的用户权限运行。可用变量：$INST_ID、$INST_NAME、$INST_DIR、$INST_JAVA、$INST_MC_VERSION、$INST_LOADER。"),
                        fields: vec![
                            FieldSpec {
                                label: "启动前命令",
                                help: Some("失败（退出码不为 0）会取消这次启动。"),
                                placeholder: "跟随默认",
                                value: shown(&current.launch.pre_launch),
                                kind: FieldKind::Line,
                            },
                            FieldSpec {
                                label: "包装命令",
                                help: Some("放在 Java 命令前面。"),
                                placeholder: "跟随默认",
                                value: shown(&current.launch.wrapper),
                                kind: FieldKind::Line,
                            },
                            FieldSpec {
                                label: "退出后命令",
                                help: Some("失败只会记录，最长运行 60 秒。"),
                                placeholder: "跟随默认",
                                value: shown(&current.launch.post_exit),
                                kind: FieldKind::Line,
                            },
                        ],
                        parse: {
                            let current = current.clone();
                            Rc::new(move |values| {
                                commands(&current, &values[0], &values[1], &values[2])
                            })
                        },
                        reset: cleared(&current, |next| {
                            next.launch.pre_launch = None;
                            next.launch.wrapper = None;
                            next.launch.post_exit = None;
                        }),
                    }
                })),
                colors,
            )
        };
        kit::list(vec![args_row, env_row, commands_row], colors).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved(result: Saved) -> InstanceSettings {
        match result.unwrap() {
            InstanceIntent::SaveSettings(settings) => settings,
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn rows_say_where_a_value_comes_from() {
        assert_eq!(followed(None, "1280 × 720"), "跟随默认 · 1280 × 720");
        assert_eq!(
            followed(Some("800 × 600".into()), "1280 × 720"),
            "800 × 600"
        );
        assert_eq!(
            window_text(Some(800), Some(600)).as_deref(),
            Some("800 × 600")
        );
        assert_eq!(window_text(Some(800), None), None);
        assert_eq!(quick_play_text(None), "无（进入主菜单）");
        assert_eq!(
            quick_play_text(Some(&QuickPlay::World("My World".into()))),
            "世界 · My World"
        );
        assert_eq!(
            command_text(Some(&String::new()), Some(&"nice".to_owned())),
            "不使用"
        );
        assert_eq!(
            command_text(None, Some(&"nice".to_owned())),
            "跟随默认 · nice"
        );
        assert_eq!(command_text(None, None), "跟随默认 · 无");
    }

    #[test]
    fn the_window_and_fullscreen_are_overrides_that_can_be_cleared() {
        let base = InstanceSettings::default();
        let next = saved(window(&base, "800", "600", 1));
        assert_eq!(
            (
                next.launch.window_width,
                next.launch.window_height,
                next.launch.fullscreen
            ),
            (Some(800), Some(600), Some(true))
        );
        // Clearing removes the override instead of storing the default.
        let back = saved(window(&next, "", "", 0));
        assert!(back.launch.is_default());
        assert!(
            window(&base, "800", "", 0)
                .unwrap_err()
                .contains("同时填宽和高")
        );
        assert!(window(&base, "x", "600", 0).is_err());
        assert_eq!(
            saved(window(&base, "", "", 2)).launch.fullscreen,
            Some(false)
        );
    }

    #[test]
    fn a_direct_start_needs_a_usable_target() {
        let base = InstanceSettings::default();
        assert_eq!(
            saved(quick_play(&base, 1, " My World ")).launch.quick_play,
            Some(QuickPlay::World("My World".into()))
        );
        assert_eq!(
            saved(quick_play(&base, 2, "mc.example.com:25565"))
                .launch
                .quick_play,
            Some(QuickPlay::Server("mc.example.com:25565".into()))
        );
        assert!(
            saved(quick_play(&base, 0, "ignored"))
                .launch
                .quick_play
                .is_none()
        );
        assert!(quick_play(&base, 1, "").unwrap_err().contains("请填写"));
        assert!(quick_play(&base, 1, "../x").is_err());
        assert!(quick_play(&base, 2, "host:99999").is_err());
    }

    #[test]
    fn lists_follow_the_defaults_or_are_set_even_when_empty() {
        let base = InstanceSettings::default();
        assert_eq!(
            saved(game_arguments(&base, 0, "--demo"))
                .launch
                .game_arguments,
            None
        );
        assert_eq!(
            saved(game_arguments(&base, 1, " --demo \n"))
                .launch
                .game_arguments,
            Some(vec!["--demo".to_owned()])
        );
        assert_eq!(
            saved(game_arguments(&base, 1, "")).launch.game_arguments,
            Some(Vec::new()),
            "own and empty means none on purpose"
        );
        let env = saved(environment(&base, 1, "A=1\nB = two"));
        assert_eq!(env.launch.environment.as_ref().unwrap()[1].value, "two");
        assert_eq!(saved(environment(&env, 0, "A=1")).launch.environment, None);
        assert!(
            environment(&base, 1, "oops")
                .unwrap_err()
                .contains("第 1 行")
        );
        assert!(
            environment(&base, 1, "=x")
                .unwrap_err()
                .contains("环境变量名")
        );
        assert_eq!(
            saved(jvm_arguments(&base, "-Da\n\n-Db")).jvm_arguments,
            ["-Da", "-Db"]
        );
        assert!(
            saved(jvm_arguments(&saved(jvm_arguments(&base, "-Da")), ""))
                .jvm_arguments
                .is_empty()
        );
    }

    #[test]
    fn a_command_field_follows_when_blank_and_dash_means_none() {
        let base = InstanceSettings::default();
        let next = saved(commands(&base, "  ", "nice", "-"));
        assert_eq!(next.launch.pre_launch, None);
        assert_eq!(next.launch.wrapper.as_deref(), Some("nice"));
        assert_eq!(next.launch.post_exit.as_deref(), Some(""));
        assert!(
            commands(&base, "", "unclosed \"quote", "")
                .unwrap_err()
                .contains("引号")
        );
    }

    #[test]
    fn java_and_after_launch_can_be_set_and_cleared() {
        let base = InstanceSettings::default();
        let with = saved(java(&base, " /opt/jdk-21 "));
        assert_eq!(with.java_path, Some(PathBuf::from("/opt/jdk-21")));
        assert_eq!(saved(java(&with, "")).java_path, None);
        assert_eq!(
            saved(after_launch(&base, 2)).launch.after_launch,
            Some(AfterLaunch::Hide)
        );
        assert_eq!(
            saved(after_launch(&base, 1)).launch.after_launch,
            Some(AfterLaunch::Keep)
        );
        assert_eq!(saved(after_launch(&base, 0)).launch.after_launch, None);
    }
}
