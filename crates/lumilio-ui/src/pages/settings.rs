//! Settings (`docs/ia/settings.md`): launcher preferences and the defaults
//! every game starts from. The page shows read-only values and quick
//! controls; anything typed is edited in a dialog (design language §10).

use std::rc::Rc;

use crate::key::Key;
use gpui::{AnyElement, App, IntoElement, SharedString, Window, div, prelude::*, px};
use gpui_component::{Sizable as _, h_flex, v_flex};
use lumilio_core::{
    AfterLaunch, Appearance, MotionPreference, Preferences, StorageUsage, recommended_memory_mb,
};

use super::live::LiveCtx;
use crate::kit::{self, ViewIntent};
use crate::live::{LiveHandler, LiveIntent, SettingsView};
use crate::settings_dialog::{DialogSpec, FieldKind, FieldSpec, SettingsDialog};
use crate::settings_forms as forms;
use crate::theme::{self, ShellColors};

pub const TABS: [&str; 5] = ["通用", "游戏默认", "Java", "下载与存储", "关于"];

/// The view-state group that remembers the chosen tab.
pub const TAB_GROUP: u8 = 201;

const APPEARANCES: [&str; 3] = ["跟随系统", "浅色", "深色"];
const AFTER_LAUNCH: [&str; 2] = ["保持", "隐藏启动器"];
const MOTION: [&str; 3] = ["跟随系统", "减少", "完整"];
const FULLSCREEN: [&str; 3] = ["关", "开", "不设置"];

// ── display text ────────────────────────────────────────────────────────

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

fn memory_help(total: Option<u64>) -> String {
    match total {
        Some(total) => format!(
            "本机内存 {}，推荐最大内存 {} MB。留空则交给 Java 决定；每个游戏还可以单独设置。",
            bytes_text(total * 1024 * 1024),
            recommended_memory_mb(total)
        ),
        None => "留空则交给 Java 决定；每个游戏还可以单独设置。".to_owned(),
    }
}

// ── controls ────────────────────────────────────────────────────────────

fn send(handler: &LiveHandler, intent: LiveIntent) -> impl Fn(&mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |window, cx| handler(intent.clone(), window, cx)
}

/// An [编辑] button that opens a dialog built when it is pressed.
fn edit(
    id: &'static str,
    handler: &LiveHandler,
    spec: impl Fn() -> DialogSpec<LiveIntent> + 'static,
) -> AnyElement {
    let handler = handler.clone();
    kit::ghost(id, "编辑", move |window, cx| {
        SettingsDialog::open(spec(), handler.clone(), window, cx);
    })
    .debug_selector(move || id.to_owned())
    .into_any_element()
}

fn row(
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

fn preference_row(
    id: &'static str,
    label: &'static str,
    labels: &'static [&'static str],
    selected: usize,
    on_pick: impl Fn(usize) -> LiveIntent + 'static,
    ctx: &LiveCtx,
) -> gpui::Stateful<gpui::Div> {
    let handler = ctx.handler.clone();
    let control = h_flex()
        .child(kit::segments(
            id,
            labels,
            selected,
            move |index, window, cx| handler(on_pick(index), window, cx),
        ))
        .into_any_element();
    row(id, label, None, "", Some(control), ctx.colors)
}

fn with_preferences(
    view: &SettingsView,
    change: impl FnOnce(&mut Preferences) + 'static,
) -> LiveIntent {
    let mut next = view.preferences.clone();
    change(&mut next);
    LiveIntent::SetPreferences(next)
}

// ── tabs ────────────────────────────────────────────────────────────────

fn general(view: &SettingsView, ctx: &LiveCtx) -> AnyElement {
    let colors = ctx.colors;
    let prefs = &view.preferences;
    let appearance = match prefs.appearance {
        Appearance::System => 0,
        Appearance::Light => 1,
        Appearance::Dark => 2,
    };
    let after = match prefs.after_launch {
        AfterLaunch::Keep => 0,
        AfterLaunch::Hide => 1,
    };
    let motion = match prefs.motion {
        MotionPreference::System => 0,
        MotionPreference::Reduce => 1,
        MotionPreference::Full => 2,
    };
    let foreground = prefs.foreground_on_exit();
    let shown = view.clone();
    let rows = vec![
        preference_row(
            "settings-appearance",
            "外观",
            &APPEARANCES,
            appearance,
            {
                let view = shown.clone();
                move |index| {
                    with_preferences(&view, move |prefs| {
                        prefs.appearance =
                            [Appearance::System, Appearance::Light, Appearance::Dark][index.min(2)];
                    })
                }
            },
            ctx,
        ),
        preference_row(
            "settings-after-launch",
            "进入游戏后",
            &AFTER_LAUNCH,
            after,
            {
                let view = shown.clone();
                move |index| {
                    with_preferences(&view, move |prefs| {
                        prefs.after_launch = [AfterLaunch::Keep, AfterLaunch::Hide][index.min(1)];
                    })
                }
            },
            ctx,
        ),
        row(
            "settings-foreground",
            "游戏退出后回到前台",
            Some("游戏结束时把启动器带回最前面，方便接着选下一个游戏。".to_owned()),
            "",
            Some(
                kit::switch(
                    "settings-foreground-switch",
                    foreground,
                    "游戏退出后回到前台",
                    send(
                        &ctx.handler,
                        with_preferences(view, move |prefs| {
                            prefs.foreground_on_exit = Some(!foreground);
                        }),
                    ),
                )
                .into_any_element(),
            ),
            colors,
        ),
        preference_row(
            "settings-motion",
            "减少动效",
            &MOTION,
            motion,
            {
                let view = shown;
                move |index| {
                    with_preferences(&view, move |prefs| {
                        prefs.motion = [
                            MotionPreference::System,
                            MotionPreference::Reduce,
                            MotionPreference::Full,
                        ][index.min(2)];
                    })
                }
            },
            ctx,
        ),
        row(
            "settings-language",
            "语言",
            Some("目前只有简体中文。".to_owned()),
            "简体中文",
            None,
            colors,
        ),
    ];
    kit::list(rows, colors).into_any_element()
}

fn game_defaults(view: &SettingsView, ctx: &LiveCtx) -> AnyElement {
    let colors = ctx.colors;
    let handler = &ctx.handler;
    let launch = view.launch.clone();
    let (min, max) = (view.min_memory_mb, view.max_memory_mb);
    let total = view.total_memory_mb;
    let recommended = total.map(recommended_memory_mb);

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

fn java(view: &SettingsView, ctx: &LiveCtx) -> AnyElement {
    let colors = ctx.colors;
    let handler = &ctx.handler;
    let list = if view.java.is_empty() {
        kit::empty(
            "没有找到 Java",
            "点“添加 Java”选一个，或在额外搜索目录里加入它所在的文件夹。",
            colors,
        )
        .into_any_element()
    } else {
        kit::list(
            view.java
                .iter()
                .enumerate()
                .map(|(index, java)| {
                    let enabled = !java.disabled;
                    let toggle = kit::switch(
                        ("settings-java-switch", index),
                        enabled,
                        "启用这个 Java",
                        send(
                            handler,
                            LiveIntent::SetJavaDisabled {
                                home: java.home.clone(),
                                disabled: enabled,
                            },
                        ),
                    );
                    let more = kit::more_menu(
                        ("settings-java-more", index),
                        vec![kit::MenuEntry::new(
                            "在访达中显示",
                            send(handler, LiveIntent::Reveal(java.home.clone())),
                        )],
                        colors,
                    );
                    kit::row(
                        if java.disabled {
                            format!("{}（已停用）", java.title)
                        } else {
                            java.title.clone()
                        },
                        java.detail.clone(),
                        None,
                        Some(
                            h_flex()
                                .gap_3()
                                .items_center()
                                .child(toggle)
                                .child(more)
                                .into_any_element(),
                        ),
                        colors,
                    )
                })
                .collect(),
            colors,
        )
        .into_any_element()
    };
    let roots: Vec<String> = view
        .java_roots
        .iter()
        .map(|root| root.display().to_string())
        .collect();
    let roots_text = roots.join("\n");
    let roots_row = row(
        "settings-java-roots",
        "额外搜索目录",
        Some("除了常见位置，也会在这些文件夹里找 Java。".to_owned()),
        if roots.is_empty() {
            "无".to_owned()
        } else {
            format!("{} 个", roots.len())
        },
        Some(edit("settings-java-roots-edit", handler, move || {
            DialogSpec {
                title: "额外搜索目录",
                intro: Some("每行一个文件夹。每个游戏会按需要自动选择合适的 Java。"),
                fields: vec![FieldSpec {
                    label: "文件夹",
                    help: None,
                    placeholder: "/opt/jdks",
                    value: roots_text.clone(),
                    kind: FieldKind::Lines { rows: 4 },
                }],
                parse: Rc::new(|values| forms::java_roots(&values[0])),
                reset: Some(LiveIntent::SetJavaRoots(Vec::new())),
            }
        })),
        colors,
    );
    v_flex()
        .w_full()
        .gap_5()
        .child(kit::section("已发现的 Java", colors, list))
        .child(
            h_flex()
                .gap_2()
                .child(kit::action(
                    "settings-java-rescan",
                    "重新检测",
                    Some(crate::assets::UiIcon::Refresh),
                    false,
                    send(handler, LiveIntent::LoadSettings),
                ))
                .child(kit::action(
                    "settings-java-install",
                    "下载推荐的 Java",
                    Some(crate::assets::UiIcon::Download),
                    false,
                    send(handler, LiveIntent::InstallJava(None)),
                ))
                .child(kit::action(
                    "settings-java-add",
                    "添加 Java…",
                    Some(crate::assets::UiIcon::Plus),
                    false,
                    send(handler, LiveIntent::AddJava),
                )),
        )
        .child(kit::list(vec![roots_row], colors))
        .into_any_element()
}

fn storage_bar(usage: &StorageUsage, colors: ShellColors) -> AnyElement {
    let total = usage.total().max(1) as f32;
    let parts = [
        ("游戏", usage.games, colors.primary),
        ("共享资源", usage.shared, kit::tone_ok()),
        ("Java", usage.runtimes, kit::tone_warn()),
        ("缓存", usage.cache, colors.muted),
    ];
    v_flex()
        .gap_3()
        .child(
            h_flex()
                .w_full()
                .h(px(10.))
                .rounded_full()
                .overflow_hidden()
                .bg(colors.surface_subtle)
                .children(parts.iter().filter(|(_, bytes, _)| *bytes > 0).map(
                    |(_, bytes, tone)| {
                        div()
                            .h_full()
                            .w(gpui::relative(*bytes as f32 / total))
                            .bg(*tone)
                    },
                )),
        )
        .child(
            h_flex()
                .gap_5()
                .flex_wrap()
                .children(parts.iter().map(|(label, bytes, tone)| {
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(div().size(px(8.)).rounded_full().bg(*tone))
                        .child(
                            div()
                                .text_sm()
                                .text_color(colors.foreground)
                                .child(format!("{label} {}", bytes_text(*bytes))),
                        )
                })),
        )
        .into_any_element()
}

fn downloads(view: &SettingsView, ctx: &LiveCtx) -> AnyElement {
    let colors = ctx.colors;
    let handler = &ctx.handler;
    let mirror_lines: Vec<String> = view
        .mirrors
        .iter()
        .map(|rule| format!("{} => {}", rule.official_prefix, rule.mirror_prefix))
        .collect();
    let prefer = view.prefer_mirrors;
    let mirror_text = mirror_lines.join("\n");
    let rules_now = view.mirrors.clone();
    let concurrency_now = view.download_concurrency;

    let prefer_row = row(
        "settings-prefer-mirrors",
        "优先使用镜像",
        Some("先试镜像地址，不通再回到官方地址。".to_owned()),
        "",
        Some(
            kit::switch(
                "settings-prefer-switch",
                prefer,
                "优先使用镜像",
                send(
                    handler,
                    LiveIntent::SetMirrors {
                        mirrors: rules_now,
                        prefer: !prefer,
                    },
                ),
            )
            .into_any_element(),
        ),
        colors,
    );
    let rules_row = row(
        "settings-mirrors",
        "镜像规则",
        Some("把官方地址的开头换成镜像地址的开头。".to_owned()),
        if mirror_lines.is_empty() {
            "无".to_owned()
        } else {
            format!("{} 条", mirror_lines.len())
        },
        Some(edit("settings-mirrors-edit", handler, move || DialogSpec {
            title: "镜像规则",
            intro: Some("每行一条，写成 官方前缀 => 镜像前缀。"),
            fields: vec![FieldSpec {
                label: "规则",
                help: None,
                placeholder: "https://libraries.minecraft.net/ => https://mirror.example/libraries/",
                value: mirror_text.clone(),
                kind: FieldKind::Lines { rows: 4 },
            }],
            parse: Rc::new(move |values| forms::mirrors(&values[0], prefer)),
            reset: Some(LiveIntent::SetMirrors {
                mirrors: Vec::new(),
                prefer,
            }),
        })),
        colors,
    );
    let concurrency_row = row(
        "settings-concurrency",
        "同时下载数",
        Some("同时下载的文件个数，1 到 32。网络不稳时调小。".to_owned()),
        concurrency_now.map_or_else(|| "自动".to_owned(), |count| count.to_string()),
        Some(edit("settings-concurrency-edit", handler, move || {
            DialogSpec {
                title: "同时下载数",
                intro: Some("留空则由启动器决定。"),
                fields: vec![FieldSpec {
                    label: "个数（1–32）",
                    help: None,
                    placeholder: "自动",
                    value: concurrency_now.map(|v| v.to_string()).unwrap_or_default(),
                    kind: FieldKind::Line,
                }],
                parse: Rc::new(|values| forms::concurrency(&values[0])),
                reset: Some(LiveIntent::SetConcurrency(None)),
            }
        })),
        colors,
    );
    let data_row = row(
        "settings-data-dir",
        "数据目录",
        Some("游戏、共享资源和设置都放在这里，不能更改。".to_owned()),
        view.data_dir.display().to_string(),
        Some(
            kit::ghost(
                "settings-reveal-data",
                "在访达中显示",
                send(handler, LiveIntent::Reveal(view.data_dir.clone())),
            )
            .into_any_element(),
        ),
        colors,
    );
    let usage = match &view.storage {
        Some(usage) => v_flex()
            .gap_4()
            .p_4()
            .child(storage_bar(usage, colors))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Key::new("settings-reclaim")
                            .label("检查没用的游戏文件")
                            .white()
                            .small()
                            .debug_selector(|| "settings-reclaim".into())
                            .on_click({
                                let handler = handler.clone();
                                move |_, window, cx| {
                                    handler(LiveIntent::CheckReclaimable, window, cx)
                                }
                            }),
                    )
                    .child(theme::clickable(
                        Key::new("settings-clear-cache")
                            .label(format!("清理缓存（{}）", bytes_text(usage.cache)))
                            .white()
                            .small()
                            .disabled(usage.cache == 0)
                            .on_click({
                                let handler = handler.clone();
                                move |_, window, cx| handler(LiveIntent::ClearCache, window, cx)
                            }),
                        usage.cache > 0,
                    )),
            )
            .into_any_element(),
        None => div()
            .p_4()
            .text_sm()
            .text_color(colors.muted)
            .child("正在计算占用…")
            .into_any_element(),
    };
    v_flex()
        .w_full()
        .gap_5()
        .child(kit::section_at(
            1,
            "下载",
            colors,
            kit::list(vec![prefer_row, rules_row, concurrency_row], colors),
        ))
        .child(kit::section_at(
            2,
            "存储",
            colors,
            kit::list(vec![data_row], colors),
        ))
        .child(kit::section_at(
            3,
            "占用",
            colors,
            kit::surface(colors).child(usage),
        ))
        .into_any_element()
}

fn about(view: &SettingsView, ctx: &LiveCtx) -> AnyElement {
    let colors = ctx.colors;
    let handler = &ctx.handler;
    let log = view.data_dir.join("activity.jsonl");
    let rows = vec![
        row(
            "settings-version",
            "版本",
            None,
            format!("LumilioCL {}", env!("CARGO_PKG_VERSION")),
            None,
            colors,
        ),
        row(
            "settings-updates",
            "检查更新",
            Some("启动器自动更新还没有提供。".to_owned()),
            "暂未提供",
            None,
            colors,
        ),
        row(
            "settings-logs",
            "启动器日志",
            Some("下载与安装的记录。".to_owned()),
            "",
            Some(
                kit::ghost(
                    "settings-reveal-log",
                    "在访达中显示",
                    send(handler, LiveIntent::Reveal(log)),
                )
                .into_any_element(),
            ),
            colors,
        ),
        row(
            "settings-diagnostics",
            "诊断包",
            Some(
                "打包版本、设置摘要、Java 列表和各游戏的最近日志；玩家名、UUID 和你的文件夹路径会被替换，启动前/包装/退出后命令只记录有没有填。"
                    .to_owned(),
            ),
            "",
            Some(
                kit::ghost(
                    "settings-export",
                    "导出…",
                    send(handler, LiveIntent::ExportDiagnostics),
                )
                .into_any_element(),
            ),
            colors,
        ),
        row(
            "settings-license",
            "开源许可",
            None,
            "AGPL-3.0",
            None,
            colors,
        ),
    ];
    kit::list(rows, colors).into_any_element()
}

pub fn render(ctx: &LiveCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let tab = ctx.state.choice(TAB_GROUP, 0).min(TABS.len() - 1);
    let body = match &ctx.model.settings {
        None => kit::empty("正在读取设置…", "", colors).into_any_element(),
        Some(view) => match tab {
            0 => general(view, ctx),
            1 => game_defaults(view, ctx),
            2 => java(view, ctx),
            3 => downloads(view, ctx),
            _ => about(view, ctx),
        },
    };
    v_flex()
        .w_full()
        .gap_5()
        .child(kit::header(
            "设置",
            "启动器本身，以及每个游戏默认使用的值",
            None,
            colors,
        ))
        .child(kit::toolbar(
            Some(
                kit::tabs("settings-tabs", &TABS, tab, {
                    let emit = ctx.emit.clone();
                    move |index, window, app| {
                        emit(ViewIntent::Choose(TAB_GROUP, index), window, app)
                    }
                })
                .into_any_element(),
            ),
            None,
        ))
        .child(kit::entrance(body, ("settings-body", tab)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumilio_core::LaunchTuning;

    #[test]
    fn values_read_in_plain_words() {
        assert_eq!(memory_text(None), "未设置");
        assert_eq!(memory_text(Some(4096)), "4096 MB");
        assert_eq!(list_text(&[]), "无");
        assert_eq!(list_text(&["-Da".into()]), "-Da");
        assert_eq!(
            list_text(&["-Da".into(), "-Db".into(), "-Dc".into()]),
            "-Da 等 3 项"
        );
        assert_eq!(bytes_text(0), "0 B");
        assert_eq!(bytes_text(2048), "2 KB");
        assert_eq!(bytes_text(5 * 1024 * 1024), "5 MB");
        assert_eq!(bytes_text(3 * 1024 * 1024 * 1024 / 2), "1.5 GB");
    }

    #[test]
    fn the_window_and_commands_are_summarized() {
        let mut view = SettingsView::default();
        assert_eq!(window_text(&view), "未设置");
        assert_eq!(commands_text(&view), "无");
        view.launch = LaunchTuning {
            window_width: Some(1280),
            window_height: Some(720),
            fullscreen: Some(true),
            wrapper: Some("nice".into()),
            post_exit: Some("true".into()),
            ..LaunchTuning::default()
        };
        assert_eq!(window_text(&view), "1280 × 720 · 全屏");
        assert_eq!(commands_text(&view), "包装、退出后");
    }

    #[test]
    fn the_memory_help_names_the_machine_when_known() {
        assert!(memory_help(Some(16_384)).contains("16.0 GB"));
        assert!(memory_help(Some(16_384)).contains("8192 MB"));
        assert!(!memory_help(None).contains("本机内存"));
    }
}
