use super::super::live::LiveCtx;
use super::rows::{edit, row, send};
use crate::kit;
use crate::live::{LiveIntent, SettingsView};
use crate::settings_dialog::{DialogSpec, FieldKind, FieldSpec};
use crate::settings_forms as forms;
use gpui::prelude::*;
use gpui::{AnyElement, IntoElement};
use gpui_component::{h_flex, v_flex};
use std::rc::Rc;

pub(super) fn java(view: &SettingsView, ctx: &LiveCtx) -> AnyElement {
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
                    // ia[settings]: Java：停用 / 启用 | Java · 列表行开关 | 停用不删除文件，之后不会被自动选中
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
                    // ia[settings]: Java：在访达中显示 | Java · 列表行 ⋯ 菜单 | 打开并选中它所在位置
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
    // ia[settings]: Java：额外搜索目录 | Java · 值 + [编辑] 弹窗 | 每行一个文件夹，检测时也会找这些位置
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
        // ia[settings]: Java：检测与列表 | Java · 已发现的 Java 列表 | 列出检测到的 Java（版本、发行方、架构、路径）；没有时提示添加
        .child(kit::section("已发现的 Java", colors, list))
        .child(
            h_flex()
                .gap_2()
                // ia[settings]: Java：重新检测 | Java · 按键「重新检测」 | 重新读取设置并检测
                .child(kit::action(
                    "settings-java-rescan",
                    "重新检测",
                    Some(crate::assets::UiIcon::Refresh),
                    false,
                    send(handler, LiveIntent::LoadSettings),
                ))
                // ia[settings]: Java：下载推荐的 Java | Java · 按键「下载推荐的 Java」 | 读 Mojang 的运行时索引，装好后出现在列表里；进度在动态 | ADR 0014
                .child(kit::action(
                    "settings-java-install",
                    "下载推荐的 Java",
                    Some(crate::assets::UiIcon::Download),
                    false,
                    send(handler, LiveIntent::InstallJava(None)),
                ))
                // ia[settings]: Java：添加 | Java · 按键「添加 Java…」→ 选 java 程序或 JDK 文件夹 | 加入列表
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
