use super::super::live::LiveCtx;
use super::rows::{edit, row, send};
use super::text::bytes_text;
use crate::key::Key;
use crate::live::{LiveIntent, SettingsView};
use crate::settings_dialog::{DialogSpec, FieldKind, FieldSpec};
use crate::settings_forms as forms;
use crate::theme::ShellColors;
use crate::{kit, theme};
use gpui::prelude::*;
use gpui::{AnyElement, IntoElement, div, px};
use gpui_component::Sizable as _;
use gpui_component::{h_flex, v_flex};
use lumilio_core::{MirrorPreset, StorageUsage};
use std::rc::Rc;

pub(super) fn storage_bar(usage: &StorageUsage, colors: ShellColors) -> AnyElement {
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

fn preset_row(
    id: &'static str,
    label: &'static str,
    help: &'static str,
    preset: MirrorPreset,
    view: &SettingsView,
    ctx: &LiveCtx,
) -> gpui::Stateful<gpui::Div> {
    let merged = preset.merge(&view.mirrors);
    let added = merged == view.mirrors;
    row(
        id,
        label,
        Some(help.to_owned()),
        if added { "已添加" } else { "未添加" },
        Some(
            kit::ghost(
                (id, 0usize),
                "添加",
                send(&ctx.handler, LiveIntent::AddMirrorPreset(preset)),
            )
            .disabled(added)
            .debug_selector(move || format!("{id}-add"))
            .into_any_element(),
        ),
        ctx.colors,
    )
}

pub(super) fn downloads(view: &SettingsView, ctx: &LiveCtx) -> AnyElement {
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

    // ia[settings]: 添加 BMCLAPI 镜像 | 下载与存储 · 预设行 [添加] | 添加游戏资源、加载器与 authlib-injector 镜像；保留已有规则和优先顺序，完整添加后禁用按钮
    let bmclapi = preset_row(
        "settings-bmclapi",
        "BMCLAPI",
        "游戏资源、Forge、NeoForge、Fabric 和 authlib-injector。添加后可打开“优先使用镜像”。",
        MirrorPreset::Bmclapi,
        view,
        ctx,
    );
    // ia[settings]: 添加 MCIM 镜像 | 下载与存储 · 预设行 [添加] | 添加 Modrinth / CurseForge 镜像；保留已有规则和优先顺序，完整添加后禁用按钮
    let mcim = preset_row(
        "settings-mcim",
        "MCIM",
        "Modrinth 和 CurseForge 的信息与文件。",
        MirrorPreset::Mcim,
        view,
        ctx,
    );
    // ia[settings]: 添加腾讯 Maven 镜像 | 下载与存储 · 预设行 [添加] | 添加 Maven Central 镜像；保留已有规则和优先顺序，完整添加后禁用按钮
    let maven = preset_row(
        "settings-tencent-maven",
        "腾讯 Maven",
        "Maven Central 上的 Java 依赖。",
        MirrorPreset::TencentMaven,
        view,
        ctx,
    );

    // ia[settings]: 优先使用镜像 | 下载与存储 · 开关 | 先试镜像地址，不通再回到官方地址
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
    // ia[settings]: 镜像规则 | 下载与存储 · 值 + [编辑] 弹窗 | 每行 官方前缀 => 镜像前缀
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
    // ia[settings]: 同时下载数 | 下载与存储 · 值 + [编辑] 弹窗 | 自动 / 1–32
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
    // ia[settings]: 数据目录 | 下载与存储 · 路径 + [在访达中显示] | 不可改（App State）
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
    // ia[settings]: 存储占用 | 下载与存储 · 游戏 / 共享资源 / Java / 缓存 条形图 | 后台计算后显示
    let usage = match &view.storage {
        Some(usage) => v_flex()
            .gap_4()
            .p_4()
            .child(storage_bar(usage, colors))
            .child(
                h_flex()
                    .gap_2()
                    // ia[settings]: 检查没用的游戏文件 | 下载与存储 · 按键「检查没用的游戏文件」 | 只报告能回收多少，确认后才删；游戏在安装/更新时拒绝 | ADR 0017
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
                    // ia[settings]: 清理缓存 | 下载与存储 · 按键「清理缓存」 | 缓存 = 已解压的 natives 与 cache/，下次启动会重建
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
            kit::list(
                vec![bmclapi, mcim, maven, prefer_row, rules_row, concurrency_row],
                colors,
            ),
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
