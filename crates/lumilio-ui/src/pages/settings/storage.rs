use super::super::live::LiveCtx;
use super::rows::{edit, row, send};
use super::text::bytes_text;
use crate::key::Key;
use crate::live::{LiveIntent, SettingsView};
use crate::settings_dialog::{DialogSpec, FieldKind, FieldSpec};
use crate::settings_forms as forms;
use crate::theme::ShellColors;
use crate::{kit, theme};
use crate::{tr, tr_all};
use gpui::prelude::*;
use gpui::{AnyElement, IntoElement, div, px};
use gpui_component::Sizable as _;
use gpui_component::{h_flex, v_flex};
use lumilio_core::{DownloadSourcePreference, MirrorPreset, StorageUsage};
use std::rc::Rc;

pub(super) fn storage_bar(usage: &StorageUsage, colors: ShellColors) -> AnyElement {
    let total = usage.total().max(1) as f32;
    let parts = [
        (tr!("settings-storage-games"), usage.games, colors.primary),
        (tr!("settings-storage-shared"), usage.shared, kit::tone_ok()),
        (
            tr!("settings-storage-java"),
            usage.runtimes,
            kit::tone_warn(),
        ),
        (tr!("settings-storage-cache"), usage.cache, colors.muted),
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
        if added {
            tr!("settings-mirror-added")
        } else {
            tr!("settings-mirror-not-added")
        },
        Some(
            kit::ghost(
                (id, 0usize),
                tr!("common-add"),
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
    let mirror_text = mirror_lines.join("\n");
    let concurrency_now = view.download_concurrency;

    // ia[settings]: 添加 BMCLAPI 镜像 | 下载与存储 · 预设行 [添加] | 添加游戏资源、加载器与 authlib-injector 镜像；保留已有规则和优先顺序，完整添加后禁用按钮
    let bmclapi = preset_row(
        "settings-bmclapi",
        "BMCLAPI",
        tr!("settings-bmclapi-help"),
        MirrorPreset::Bmclapi,
        view,
        ctx,
    );
    // ia[settings]: 添加 MCIM 镜像 | 下载与存储 · 预设行 [添加] | 添加 Modrinth / CurseForge 镜像；保留已有规则和优先顺序，完整添加后禁用按钮
    let mcim = preset_row(
        "settings-mcim",
        "MCIM",
        tr!("settings-mcim-help"),
        MirrorPreset::Mcim,
        view,
        ctx,
    );
    // ia[settings]: 添加腾讯 Maven 镜像 | 下载与存储 · 预设行 [添加] | 添加 Maven Central 镜像；保留已有规则和优先顺序，完整添加后禁用按钮
    let maven = preset_row(
        "settings-tencent-maven",
        tr!("settings-tencent-maven"),
        tr!("settings-tencent-maven-help"),
        MirrorPreset::TencentMaven,
        view,
        ctx,
    );

    let source_choices = [
        DownloadSourcePreference::OfficialOnly,
        DownloadSourcePreference::OfficialFirst,
        DownloadSourcePreference::MirrorFirst,
    ];
    let selected = source_choices
        .iter()
        .position(|choice| *choice == view.download_source)
        .unwrap_or(1);
    // ia[settings]: 下载源 | 下载与存储 · 分段：仅官方 / 官方优先 / 镜像优先 | 立即保存；仅官方不访问镜像，其他模式按顺序回退，MCIM 始终位于官方之后
    let prefer_row = row(
        "settings-download-source",
        tr!("settings-download-source"),
        Some(tr!("settings-download-source-help").to_owned()),
        "",
        Some(
            kit::segments(
                "settings-download-source",
                tr_all![
                    "settings-source-official-only",
                    "settings-source-official-first",
                    "settings-source-mirror-first"
                ],
                selected,
                {
                    let handler = handler.clone();
                    move |index, window, cx| {
                        handler(
                            LiveIntent::SetDownloadSource(source_choices[index.min(2)]),
                            window,
                            cx,
                        )
                    }
                },
            )
            .into_any_element(),
        ),
        colors,
    );
    // ia[settings]: 镜像规则 | 下载与存储 · 值 + [编辑] 弹窗 | 每行 官方前缀 => 镜像前缀
    let rules_row = row(
        "settings-mirrors",
        tr!("settings-mirrors"),
        Some(tr!("settings-mirrors-help").to_owned()),
        if mirror_lines.is_empty() {
            tr!("common-none").to_owned()
        } else {
            tr!("settings-mirrors-count", count = mirror_lines.len())
        },
        Some(edit("settings-mirrors-edit", handler, move || DialogSpec {
            title: tr!("settings-mirrors"),
            intro: Some(tr!("settings-mirrors-intro")),
            fields: vec![FieldSpec {
                label: tr!("settings-mirrors-field"),
                help: None,
                placeholder: "https://libraries.minecraft.net/ => https://mirror.example/libraries/",
                value: mirror_text.clone(),
                kind: FieldKind::Lines { rows: 4 },
            }],
            parse: Rc::new(move |values| forms::mirrors(&values[0])),
            reset: Some(LiveIntent::SetMirrors(Vec::new())),
        })),
        colors,
    );
    // ia[settings]: 同时下载数 | 下载与存储 · 值 + [编辑] 弹窗 | 自动 / 1–32
    let concurrency_row = row(
        "settings-concurrency",
        tr!("settings-concurrency"),
        Some(tr!("settings-concurrency-help").to_owned()),
        concurrency_now.map_or_else(|| tr!("common-auto").to_owned(), |count| count.to_string()),
        Some(edit("settings-concurrency-edit", handler, move || {
            DialogSpec {
                title: tr!("settings-concurrency"),
                intro: Some(tr!("settings-concurrency-intro")),
                fields: vec![FieldSpec {
                    label: tr!("settings-concurrency-field"),
                    help: None,
                    placeholder: tr!("common-auto"),
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
        tr!("settings-data-dir"),
        Some(tr!("settings-data-dir-help").to_owned()),
        view.data_dir.display().to_string(),
        Some(
            kit::ghost(
                "settings-reveal-data",
                crate::platform::reveal_label(),
                send(handler, LiveIntent::Reveal(view.data_dir.clone())),
            )
            .into_any_element(),
        ),
        colors,
    );
    // ia[settings]: 存储占用 | 下载与存储 · 游戏 / 共享资源 / Java / 缓存 条形图 | 后台计算后显示
    // ia[settings]: 清除地图缓存 | 下载与存储 · 按键「清除地图缓存」 | 后台删除种子地图缓存；下次查看时重新计算
    let clear_map = Key::new("settings-clear-map-cache")
        .label(tr!("map-clear-cache"))
        .white()
        .small()
        .on_click({
            let handler = handler.clone();
            move |_, window, cx| handler(LiveIntent::ClearMapCache, window, cx)
        });
    let usage = match &view.storage {
        Some(usage) => v_flex()
            .gap_4()
            .p_4()
            .child(clear_map)
            .child(storage_bar(usage, colors))
            .child(
                h_flex()
                    .gap_2()
                    // ia[settings]: 检查没用的游戏文件 | 下载与存储 · 按键「检查没用的游戏文件」 | 只报告能回收多少，确认后才删；游戏在安装/更新时拒绝 | ADR 0017
                    .child(
                        Key::new("settings-reclaim")
                            .label(tr!("settings-reclaim"))
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
                            .label(tr!("settings-clear-cache", size = bytes_text(usage.cache)))
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
            .child(tr!("settings-usage-measuring"))
            .into_any_element(),
    };
    v_flex()
        .w_full()
        .gap_5()
        .child(kit::section_at(
            1,
            tr!("settings-section-downloads"),
            colors,
            kit::list(
                vec![bmclapi, mcim, maven, prefer_row, rules_row, concurrency_row],
                colors,
            ),
        ))
        .child(kit::section_at(
            2,
            tr!("settings-section-storage"),
            colors,
            kit::list(vec![data_row], colors),
        ))
        .child(kit::section_at(
            3,
            tr!("settings-section-usage"),
            colors,
            kit::surface(colors).child(usage),
        ))
        .into_any_element()
}
