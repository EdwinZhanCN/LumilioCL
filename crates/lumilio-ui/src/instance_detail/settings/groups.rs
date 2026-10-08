use super::super::{InstanceDetailView, InstanceIntent};
use super::saves::{
    after_launch, cleared, commands, environment, game_arguments, java, jvm_arguments, quick_play,
    window,
};
use super::texts::{
    NONE_ON_PURPOSE, after_launch_text, command_text, followed, quick_play_text, window_text,
};
use super::{AFTER_LAUNCH, FOLLOW_OR_OWN, FULLSCREEN, QUICK_PLAY};
use crate::pages::settings::{bytes_text, list_text};
use crate::settings_dialog::{DialogSpec, FieldKind, FieldSpec};
use crate::settings_forms as shared;
use crate::theme::ShellColors;
use crate::{kit, live, tr};
use gpui::prelude::*;
use gpui::{AnyElement, Context};
use lumilio_core::{AfterLaunch, EnvVar, LauncherSettings, QuickPlay, recommended_memory_mb};
use std::rc::Rc;

impl InstanceDetailView {
    pub(super) fn group_game(&self, colors: ShellColors) -> AnyElement {
        let record = self.record.as_ref().expect("loaded");
        let own = record.settings.clone();
        let defaults: &LauncherSettings = &self.defaults;

        let size_default = window_text(defaults.launch.window_width, defaults.launch.window_height)
            .unwrap_or_else(|| tr!("common-not-set").to_owned());
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
        let effective = if fullscreen {
            tr!("instance-settings-window-fullscreen", size = size)
        } else {
            size
        };
        let window_value = if overridden {
            tr!("instance-settings-window-custom", value = effective)
        } else {
            followed(None, effective)
        };
        let window_row = {
            let current = own.clone();
            // ia[instance.settings]: 窗口大小、全屏 | 设置 · 游戏组，值 + [编辑] 弹窗 | 宽高一起填（留空跟随默认）；全屏 关 / 开 / 跟随默认；恢复默认＝移除覆盖
            self.setting_row(
                "isettings-window",
                tr!("settings-window"),
                Some(tr!("instance-settings-window-help").to_owned()),
                window_value,
                Some(self.edit_spec("isettings-window-edit", move || DialogSpec {
                    title: tr!("settings-window"),
                    intro: Some(tr!("instance-settings-window-intro")),
                    fields: vec![
                            FieldSpec {
                                label: tr!("settings-window-width"),
                                help: None,
                                placeholder: tr!("instance-settings-follow-default"),
                                value: current
                                    .launch
                                    .window_width
                                    .map(|v| v.to_string())
                                    .unwrap_or_default(),
                                kind: FieldKind::Line,
                            },
                            FieldSpec {
                                label: tr!("settings-window-height"),
                                help: None,
                                placeholder: tr!("instance-settings-follow-default"),
                                value: current
                                    .launch
                                    .window_height
                                    .map(|v| v.to_string())
                                    .unwrap_or_default(),
                                kind: FieldKind::Line,
                            },
                            FieldSpec {
                                label: tr!("settings-window-fullscreen"),
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
            // ia[instance.settings]: 进入游戏后 | 设置 · 游戏组，值 + [编辑] 弹窗 | 保持 / 隐藏启动器 / 跟随默认
            self.setting_row(
                "isettings-after",
                tr!("settings-after-launch"),
                Some(tr!("instance-settings-after-help").to_owned()),
                followed(
                    own.launch
                        .after_launch
                        .map(|v| after_launch_text(v).to_owned()),
                    after_launch_text(defaults.preferences.after_launch),
                ),
                Some(self.edit_spec("isettings-after-edit", move || DialogSpec {
                    title: tr!("settings-after-launch"),
                    intro: None,
                    fields: vec![FieldSpec {
                        label: tr!("instance-settings-launcher-window"),
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
            // ia[instance.settings]: 直接进入（Quick Play） | 设置 · 游戏组，值 + [编辑] 弹窗：类型 + 目标 | 启动后直达世界或服务器；版本不支持单人世界时启动会说明原因；世界页的「进入」用同一个能力
            self.setting_row(
                "isettings-quick",
                tr!("instance-settings-quick-play"),
                Some(tr!("instance-settings-quick-help").to_owned()),
                quick_play_text(own.launch.quick_play.as_ref()),
                Some(self.edit_spec("isettings-quick-edit", move || {
                    let (kind, target) = match &current.launch.quick_play {
                        None => (0, String::new()),
                        Some(QuickPlay::World(name)) => (1, name.clone()),
                        Some(QuickPlay::Server(address)) => (2, address.clone()),
                    };
                    DialogSpec {
                        title: tr!("instance-settings-quick-play"),
                        intro: Some(tr!("instance-settings-quick-intro")),
                        fields: vec![
                            FieldSpec {
                                label: tr!("instance-settings-type"),
                                help: None,
                                placeholder: "",
                                value: String::new(),
                                kind: FieldKind::Choice {
                                    labels: &QUICK_PLAY,
                                    selected: kind,
                                },
                            },
                            FieldSpec {
                                label: tr!("instance-settings-target"),
                                help: None,
                                placeholder: tr!("instance-settings-target-placeholder"),
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

    pub(super) fn group_runtime(&self, colors: ShellColors, _cx: &mut Context<Self>) -> AnyElement {
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
            // ia[instance.settings]: 更换游戏版本 / 加载器 | 设置 · 运行时，「游戏版本」「加载器」行的 [更换…] → 弹窗（加载器分段 + 版本选择器，起点是游戏现在的组合） | 警告“装好的 Mod 可能不兼容，先建快照” + [先建快照]；与现在相同时不能提交；失败回到旧组合
            self.setting_row(
                "isettings-version",
                tr!("instance-field-game-version"),
                Some(tr!("instance-settings-version-help").to_owned()),
                record.game_version.clone(),
                Some(button(
                    "isettings-version-change",
                    tr!("instance-action-change"),
                    InstanceIntent::OpenRuntimeChange,
                )),
                colors,
            ),
            self.setting_row(
                "isettings-loader",
                tr!("library-loader"),
                Some(tr!("instance-settings-loader-help").to_owned()),
                match &record.loader_version {
                    Some(version) => format!("{} {version}", live::loader_label(record.loader)),
                    None => live::loader_label(record.loader).to_owned(),
                },
                Some(button(
                    "isettings-loader-change",
                    tr!("instance-action-change"),
                    InstanceIntent::OpenRuntimeChange,
                )),
                colors,
            ),
            // ia[instance.settings]: 修复游戏文件 | 设置 · 运行时，「游戏文件」行的 [修复] | 逐个核对游戏文件，缺的或损坏的重新下载；世界、Mod 和设置不会被改动；后台任务
            self.setting_row(
                "isettings-files",
                tr!("instance-settings-game-files"),
                Some(tr!("instance-settings-files-help").to_owned()),
                if record.installed {
                    tr!("discover-installed")
                } else {
                    tr!("instance-settings-not-downloaded")
                },
                Some(button(
                    "isettings-repair",
                    tr!("instance-action-repair"),
                    InstanceIntent::Repair,
                )),
                colors,
            ),
        ];
        kit::list(rows, colors).into_any_element()
    }

    pub(super) fn group_java(&self, colors: ShellColors) -> AnyElement {
        let record = self.record.as_ref().expect("loaded");
        let own = record.settings.clone();
        let java_row = {
            let current = own.clone();
            // ia[instance.settings]: 指定 Java | 设置 · Java 组，值 + [编辑] 弹窗 | 路径留空自动选择；保存时检查路径存在；已发现的 Java 列表在全局设置·Java
            self.setting_row(
                "isettings-java",
                "Java",
                Some(tr!("instance-settings-java-help").to_owned()),
                own.java_path.as_ref().map_or_else(
                    || tr!("instance-settings-java-auto").to_owned(),
                    |path| path.display().to_string(),
                ),
                Some(self.edit_spec("isettings-java-edit", move || DialogSpec {
                    title: "Java",
                    intro: Some(tr!("instance-settings-java-intro")),
                    fields: vec![FieldSpec {
                        label: tr!("instance-settings-path"),
                        help: None,
                        placeholder: tr!("instance-settings-java-auto"),
                        value: current
                            .java_path
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_default(),
                        kind: FieldKind::Path {
                            files: true,
                            directories: true,
                            prompt: tr!("instance-settings-java-pick"),
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
            // ia[instance.settings]: Java 参数 | 设置 · Java 组，值 + [编辑] 弹窗 | 多行文本，每行一个参数
            self.setting_row(
                "isettings-jvm",
                tr!("settings-jvm"),
                Some(tr!("instance-settings-jvm-help").to_owned()),
                if own.jvm_arguments.is_empty() {
                    followed(None, default)
                } else {
                    list_text(&own.jvm_arguments)
                },
                Some(self.edit_spec("isettings-jvm-edit", move || DialogSpec {
                    title: tr!("settings-jvm"),
                    intro: Some(tr!("instance-settings-jvm-intro")),
                    fields: vec![FieldSpec {
                        label: tr!("instance-settings-args"),
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

    pub(in super::super) fn machine_memory_row(
        &self,
        colors: ShellColors,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        let total = self.machine_memory_mb?;
        Some(self.setting_row(
            "isettings-machine-memory",
            tr!("instance-settings-machine-memory"),
            Some(tr!("instance-settings-machine-memory-help").to_owned()),
            tr!(
                "instance-settings-machine-memory-value",
                total = bytes_text(total * 1024 * 1024),
                recommended = recommended_memory_mb(total)
            ),
            None,
            colors,
        ))
    }

    pub(super) fn group_advanced(&self, colors: ShellColors) -> AnyElement {
        let record = self.record.as_ref().expect("loaded");
        let own = record.settings.clone();
        let defaults = &self.defaults.launch;

        let args_row = {
            let current = own.clone();
            let default = list_text(&defaults.game_arguments);
            // ia[instance.settings]: 游戏参数 | 设置 · 高级组，值 + [编辑] 弹窗 | 来源（跟随默认 / 自己设置）+ 多行
            self.setting_row(
                "isettings-game-args",
                tr!("settings-game-args"),
                Some(tr!("instance-settings-game-args-help").to_owned()),
                match &own.launch.game_arguments {
                    None => followed(None, default),
                    Some(list) if list.is_empty() => tr!("common-none").to_owned(),
                    Some(list) => list_text(list),
                },
                Some(
                    self.edit_spec("isettings-game-args-edit", move || DialogSpec {
                        title: tr!("settings-game-args"),
                        intro: Some(tr!("settings-one-per-line")),
                        fields: vec![
                            FieldSpec {
                                label: tr!("instance-settings-source"),
                                help: None,
                                placeholder: "",
                                value: String::new(),
                                kind: FieldKind::Choice {
                                    labels: &FOLLOW_OR_OWN,
                                    selected: usize::from(current.launch.game_arguments.is_some()),
                                },
                            },
                            FieldSpec {
                                label: tr!("instance-settings-args"),
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
            // ia[instance.settings]: 环境变量 | 设置 · 高级组，值 + [编辑] 弹窗 | 来源 + 每行 名称=值
            self.setting_row(
                "isettings-env",
                tr!("settings-env"),
                Some(tr!("instance-settings-env-help").to_owned()),
                match &own.launch.environment {
                    None => followed(None, default),
                    Some(list) if list.is_empty() => tr!("common-none").to_owned(),
                    Some(list) => list_text(&env_lines(list)),
                },
                Some(self.edit_spec("isettings-env-edit", move || DialogSpec {
                    title: tr!("settings-env"),
                    intro: Some(tr!("settings-env-intro")),
                    fields: vec![
                        FieldSpec {
                            label: tr!("instance-settings-source"),
                            help: None,
                            placeholder: "",
                            value: String::new(),
                            kind: FieldKind::Choice {
                                labels: &FOLLOW_OR_OWN,
                                selected: usize::from(current.launch.environment.is_some()),
                            },
                        },
                        FieldSpec {
                            label: tr!("instance-settings-variables"),
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
            let value = tr!(
                "instance-settings-commands-value",
                pre = command_text(own.launch.pre_launch.as_ref(), defaults.pre_launch.as_ref()),
                wrapper = command_text(own.launch.wrapper.as_ref(), defaults.wrapper.as_ref()),
                post = command_text(own.launch.post_exit.as_ref(), defaults.post_exit.as_ref()),
            );
            // ia[instance.settings]: 启动前 / 包装 / 退出后命令 | 设置 · 高级组，值 + [编辑] 弹窗 | 三个输入：留空跟随默认，填 - 表示这个游戏不使用；说明可用变量
            self.setting_row(
                "isettings-commands",
                tr!("settings-commands"),
                Some(tr!("instance-settings-commands-help").to_owned()),
                value,
                Some(self.edit_spec("isettings-commands-edit", move || {
                    let shown = |own: &Option<String>| match own {
                        None => String::new(),
                        Some(text) if text.is_empty() => NONE_ON_PURPOSE.to_owned(),
                        Some(text) => text.clone(),
                    };
                    DialogSpec {
                        title: tr!("settings-commands-dialog"),
                        intro: Some(tr!("instance-settings-commands-intro")),
                        fields: vec![
                            FieldSpec {
                                label: tr!("settings-command-pre"),
                                help: Some(tr!("instance-settings-command-pre-help")),
                                placeholder: tr!("instance-settings-follow-default"),
                                value: shown(&current.launch.pre_launch),
                                kind: FieldKind::Line,
                            },
                            FieldSpec {
                                label: tr!("settings-command-wrapper"),
                                help: Some(tr!("instance-settings-command-wrapper-help")),
                                placeholder: tr!("instance-settings-follow-default"),
                                value: shown(&current.launch.wrapper),
                                kind: FieldKind::Line,
                            },
                            FieldSpec {
                                label: tr!("settings-command-post"),
                                help: Some(tr!("instance-settings-command-post-help")),
                                placeholder: tr!("instance-settings-follow-default"),
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
