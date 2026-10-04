use super::super::live::LiveCtx;
use super::FULLSCREEN;
use super::rows::{edit, row};
use super::text::{commands_text, list_text, memory_help, memory_text, window_text};
use crate::kit;
use crate::live::{LiveIntent, SettingsView};
use crate::settings_dialog::{DialogSpec, FieldKind, FieldSpec};
use crate::settings_forms as forms;
use gpui::prelude::*;
use gpui::{AnyElement, IntoElement};
use gpui_component::v_flex;
use lumilio_core::recommended_memory_mb;
use std::rc::Rc;

pub(super) fn game_defaults(view: &SettingsView, ctx: &LiveCtx) -> AnyElement {
    let colors = ctx.colors;
    let handler = &ctx.handler;
    let launch = view.launch.clone();
    let (min, max) = (view.min_memory_mb, view.max_memory_mb);
    let total = view.total_memory_mb;
    let recommended = total.map(recommended_memory_mb);

    // ia[settings]: 最小 / 最大内存 | 游戏默认 · 值 + [编辑] 弹窗 | 弹窗说明本机内存与推荐值；恢复默认 = 推荐值
    let memory = row(
        "settings-memory",
        "内存",
        Some(memory_help(total)),
        format!("最小 {} · 最大 {}", memory_text(min), memory_text(max)),
        Some(edit("settings-memory-edit", handler, move || DialogSpec {
            title: "默认内存",
            intro: Some("所有游戏默认使用的内存；单个游戏可以覆盖。留空表示不限制。"),
            fields: vec![
                FieldSpec {
                    label: "最小内存（MB）",
                    help: Some("游戏启动时就占用的内存（-Xms）。"),
                    placeholder: "不设置",
                    value: min.map(|v| v.to_string()).unwrap_or_default(),
                    kind: FieldKind::Line,
                },
                FieldSpec {
                    label: "最大内存（MB）",
                    help: Some("游戏最多能用的内存（-Xmx）。"),
                    placeholder: "不设置",
                    value: max.map(|v| v.to_string()).unwrap_or_default(),
                    kind: FieldKind::Line,
                },
            ],
            parse: Rc::new(|values| forms::memory(&values[0], &values[1])),
            reset: Some(LiveIntent::SetMemory {
                min_mb: None,
                max_mb: recommended.and_then(|mb| u32::try_from(mb).ok()),
            }),
        })),
        colors,
    );

    let window_current = launch.clone();
    // ia[settings]: 窗口大小、全屏 | 游戏默认 · 值 + [编辑] 弹窗 | 宽高一起填；全屏 关 / 开 / 不设置
    let window = row(
        "settings-window",
        "窗口大小与全屏",
        None,
        window_text(view),
        Some(edit("settings-window-edit", handler, move || {
            let current = window_current.clone();
            let fullscreen = match current.fullscreen {
                Some(false) => 0,
                Some(true) => 1,
                None => 2,
            };
            DialogSpec {
                title: "窗口大小与全屏",
                intro: Some("宽和高需要一起填；都留空则用游戏自己的默认（854 × 480）。"),
                fields: vec![
                    FieldSpec {
                        label: "宽度",
                        help: None,
                        placeholder: "854",
                        value: current
                            .window_width
                            .map(|v| v.to_string())
                            .unwrap_or_default(),
                        kind: FieldKind::Line,
                    },
                    FieldSpec {
                        label: "高度",
                        help: None,
                        placeholder: "480",
                        value: current
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
                            selected: fullscreen,
                        },
                    },
                ],
                parse: {
                    let current = current.clone();
                    Rc::new(move |values| {
                        forms::window(
                            &current,
                            &values[0],
                            &values[1],
                            values[2].parse().unwrap_or(2),
                        )
                    })
                },
                reset: Some(forms::window(&current, "", "", 2).unwrap_or(LiveIntent::LoadSettings)),
            }
        })),
        colors,
    );

    let text_row = |id: &'static str,
                    edit_id: &'static str,
                    label: &'static str,
                    help: &'static str,
                    value: String,
                    initial: String,
                    placeholder: &'static str,
                    parse: crate::settings_dialog::ParseFn<LiveIntent>,
                    title: &'static str,
                    intro: &'static str| {
        row(
            id,
            label,
            Some(help.to_owned()),
            value,
            Some(edit(edit_id, handler, move || DialogSpec {
                title,
                intro: Some(intro),
                fields: vec![FieldSpec {
                    label,
                    help: None,
                    placeholder,
                    value: initial.clone(),
                    kind: FieldKind::Lines { rows: 4 },
                }],
                parse: parse.clone(),
                reset: None,
            })),
            colors,
        )
    };

    let jvm = {
        let current = launch.clone();
        // ia[settings]: Java 参数 | 游戏默认 · 值 + [编辑] 弹窗 | 每行一项；游戏自己设置了参数时以游戏的为准
        text_row(
            "settings-jvm",
            "settings-jvm-edit",
            "Java 参数",
            "传给 Java 的附加参数，例如 -XX:+UseG1GC。游戏自己设置了参数时以游戏的为准。",
            list_text(&launch.jvm_arguments),
            forms::join_lines(&launch.jvm_arguments),
            "-XX:+UseG1GC",
            Rc::new(move |values| forms::jvm_arguments(&current, &values[0])),
            "Java 参数",
            "每行一个参数。",
        )
    };
    let game = {
        let current = launch.clone();
        // ia[settings]: 游戏参数 | 游戏默认 · 值 + [编辑] 弹窗 | 每行一项
        text_row(
            "settings-game-args",
            "settings-game-args-edit",
            "游戏参数",
            "传给游戏本身的附加参数，例如 --demo。",
            list_text(&launch.game_arguments),
            forms::join_lines(&launch.game_arguments),
            "--demo",
            Rc::new(move |values| forms::game_arguments(&current, &values[0])),
            "游戏参数",
            "每行一个参数。",
        )
    };
    let environment_text = launch
        .environment
        .iter()
        .map(|variable| format!("{}={}", variable.name, variable.value))
        .collect::<Vec<_>>();
    let environment = {
        let current = launch.clone();
        // ia[settings]: 环境变量 | 游戏默认 · 值 + [编辑] 弹窗 | 每行 名称=值
        text_row(
            "settings-env",
            "settings-env-edit",
            "环境变量",
            "游戏进程启动时额外带上的环境变量。",
            list_text(&environment_text),
            environment_text.join("\n"),
            "MESA_DEBUG=1",
            Rc::new(move |values| forms::environment(&current, &values[0])),
            "环境变量",
            "每行一个，写成 名称=值。",
        )
    };

    let commands_current = launch.clone();
    // ia[settings]: 启动前 / 包装 / 退出后命令 | 游戏默认 · 值 + [编辑] 弹窗 | 以当前用户权限运行；启动前命令失败取消启动，退出后命令失败只记录
    let commands = row(
        "settings-commands",
        "启动前、包装与退出后命令",
        Some("这些命令由你自己填写，会以当前用户的权限运行。".to_owned()),
        commands_text(view),
        Some(edit("settings-commands-edit", handler, move || {
            let current = commands_current.clone();
            DialogSpec {
                title: "命令",
                intro: Some(
                    "命令以你的用户权限运行，只在你填写后才会执行。可用变量：$INST_ID、$INST_NAME、$INST_DIR（游戏目录）、$INST_JAVA、$INST_MC_VERSION、$INST_LOADER。",
                ),
                fields: vec![
                    FieldSpec {
                        label: "启动前命令",
                        help: Some("游戏启动前运行；失败（退出码不为 0）会取消这次启动。"),
                        placeholder: "例如 ./prepare.sh",
                        value: current.pre_launch.clone().unwrap_or_default(),
                        kind: FieldKind::Line,
                    },
                    FieldSpec {
                        label: "包装命令",
                        help: Some("放在 Java 命令前面，例如 gamemoderun 或 mangohud。"),
                        placeholder: "例如 gamemoderun",
                        value: current.wrapper.clone().unwrap_or_default(),
                        kind: FieldKind::Line,
                    },
                    FieldSpec {
                        label: "退出后命令",
                        help: Some("游戏退出后运行；失败只会记录，最长运行 60 秒。"),
                        placeholder: "例如 ./cleanup.sh",
                        value: current.post_exit.clone().unwrap_or_default(),
                        kind: FieldKind::Line,
                    },
                ],
                parse: {
                    let current = current.clone();
                    Rc::new(move |values| {
                        forms::commands(&current, &values[0], &values[1], &values[2])
                    })
                },
                reset: Some(
                    forms::commands(&current, "", "", "").unwrap_or(LiveIntent::LoadSettings),
                ),
            }
        })),
        colors,
    );

    v_flex()
        .w_full()
        .gap_5()
        .child(kit::section_at(
            1,
            "资源",
            colors,
            kit::list(vec![memory, window], colors),
        ))
        .child(kit::section_at(
            2,
            "参数与环境",
            colors,
            kit::list(vec![jvm, game, environment], colors),
        ))
        .child(kit::section_at(
            3,
            "命令",
            colors,
            kit::list(vec![commands], colors),
        ))
        .into_any_element()
}
