use super::super::live::LiveCtx;
use super::fullscreen_choices;
use super::rows::{edit, row};
use super::text::{commands_text, list_text, memory_help, memory_text, window_text};
use crate::kit;
use crate::live::{LiveIntent, SettingsView};
use crate::settings_dialog::{DialogSpec, FieldKind, FieldSpec};
use crate::settings_forms as forms;
use crate::tr;
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
        tr!("settings-memory"),
        Some(memory_help(total)),
        tr!(
            "settings-memory-value",
            min = memory_text(min),
            max = memory_text(max)
        ),
        Some(edit("settings-memory-edit", handler, move || DialogSpec {
            title: tr!("settings-memory-dialog"),
            intro: Some(tr!("settings-memory-intro")),
            fields: vec![
                FieldSpec {
                    label: tr!("settings-memory-min"),
                    help: Some(tr!("settings-memory-min-help")),
                    placeholder: tr!("settings-memory-placeholder"),
                    value: min.map(|v| v.to_string()).unwrap_or_default(),
                    kind: FieldKind::Line,
                },
                FieldSpec {
                    label: tr!("settings-memory-max"),
                    help: Some(tr!("settings-memory-max-help")),
                    placeholder: tr!("settings-memory-placeholder"),
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
        tr!("settings-window"),
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
                title: tr!("settings-window"),
                intro: Some(tr!("settings-window-intro")),
                fields: vec![
                    FieldSpec {
                        label: tr!("settings-window-width"),
                        help: None,
                        placeholder: "854",
                        value: current
                            .window_width
                            .map(|v| v.to_string())
                            .unwrap_or_default(),
                        kind: FieldKind::Line,
                    },
                    FieldSpec {
                        label: tr!("settings-window-height"),
                        help: None,
                        placeholder: "480",
                        value: current
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
                            labels: fullscreen_choices(),
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
            tr!("settings-jvm"),
            tr!("settings-jvm-help"),
            list_text(&launch.jvm_arguments),
            forms::join_lines(&launch.jvm_arguments),
            "-XX:+UseG1GC",
            Rc::new(move |values| forms::jvm_arguments(&current, &values[0])),
            tr!("settings-jvm"),
            tr!("settings-one-per-line"),
        )
    };
    let game = {
        let current = launch.clone();
        // ia[settings]: 游戏参数 | 游戏默认 · 值 + [编辑] 弹窗 | 每行一项
        text_row(
            "settings-game-args",
            "settings-game-args-edit",
            tr!("settings-game-args"),
            tr!("settings-game-args-help"),
            list_text(&launch.game_arguments),
            forms::join_lines(&launch.game_arguments),
            "--demo",
            Rc::new(move |values| forms::game_arguments(&current, &values[0])),
            tr!("settings-game-args"),
            tr!("settings-one-per-line"),
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
            tr!("settings-env"),
            tr!("settings-env-help"),
            list_text(&environment_text),
            environment_text.join("\n"),
            "MESA_DEBUG=1",
            Rc::new(move |values| forms::environment(&current, &values[0])),
            tr!("settings-env"),
            tr!("settings-env-intro"),
        )
    };

    let commands_current = launch.clone();
    // ia[settings]: 启动前 / 包装 / 退出后命令 | 游戏默认 · 值 + [编辑] 弹窗 | 以当前用户权限运行；启动前命令失败取消启动，退出后命令失败只记录
    let commands = row(
        "settings-commands",
        tr!("settings-commands"),
        Some(tr!("settings-commands-help").to_owned()),
        commands_text(view),
        Some(edit("settings-commands-edit", handler, move || {
            let current = commands_current.clone();
            DialogSpec {
                title: tr!("settings-commands-dialog"),
                intro: Some(tr!("settings-commands-intro")),
                fields: vec![
                    FieldSpec {
                        label: tr!("settings-command-pre"),
                        help: Some(tr!("settings-command-pre-help")),
                        placeholder: tr!("settings-command-pre-placeholder"),
                        value: current.pre_launch.clone().unwrap_or_default(),
                        kind: FieldKind::Line,
                    },
                    FieldSpec {
                        label: tr!("settings-command-wrapper"),
                        help: Some(tr!("settings-command-wrapper-help")),
                        placeholder: tr!("settings-command-wrapper-placeholder"),
                        value: current.wrapper.clone().unwrap_or_default(),
                        kind: FieldKind::Line,
                    },
                    FieldSpec {
                        label: tr!("settings-command-post"),
                        help: Some(tr!("settings-command-post-help")),
                        placeholder: tr!("settings-command-post-placeholder"),
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
            tr!("settings-section-resources"),
            colors,
            kit::list(vec![memory, window], colors),
        ))
        .child(kit::section_at(
            2,
            tr!("settings-section-arguments"),
            colors,
            kit::list(vec![jvm, game, environment], colors),
        ))
        .child(kit::section_at(
            3,
            tr!("settings-section-commands"),
            colors,
            kit::list(vec![commands], colors),
        ))
        .into_any_element()
}
