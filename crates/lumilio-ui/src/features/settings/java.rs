use std::rc::Rc;

use gpui::prelude::*;
use gpui::{AnyElement, App, IntoElement, Window};
use gpui_component::{WindowExt as _, h_flex, v_flex};
use lumilio_core::JavaSource;

use super::page::SettingsCtx;
use super::rows::{edit, preference_row, row, send};
use crate::live::{JavaRow, LiveHandler, LiveIntent, SettingsView};
use crate::settings_dialog::{DialogSpec, FieldKind, FieldSpec, SettingsDialog};
use crate::{kit, settings_forms as forms, tr, tr_all};

/// The Java a download is offered at first.
const RECOMMENDED_MAJOR: u32 = 21;

/// What Mojang offers when its list could not be read: every major a game
/// version has asked for. A major this system lacks fails with a reason.
const MOJANG_FALLBACK: [u32; 5] = [8, 16, 17, 21, 25];

fn source_index(source: JavaSource) -> usize {
    JavaSource::ALL
        .iter()
        .position(|candidate| *candidate == source)
        .unwrap_or(0)
}

/// The majors `source` offers here, from what was read, else a fallback.
fn offered(view: &SettingsView, source: JavaSource) -> Vec<u32> {
    let read = view
        .java_offers
        .iter()
        .find(|(offer, _)| *offer == source)
        .map(|(_, majors)| majors.clone())
        .filter(|majors| !majors.is_empty());
    read.unwrap_or_else(|| match source {
        JavaSource::Mojang => MOJANG_FALLBACK.to_vec(),
        JavaSource::OracleGraalVm => vec![21, 25],
    })
}

/// The 下载 Java… dialog: one dropdown of the majors the chosen source has.
fn download_spec(source: JavaSource, majors: &[u32]) -> DialogSpec<LiveIntent> {
    let options: Vec<String> = majors.iter().map(|major| format!("Java {major}")).collect();
    let selected = majors
        .iter()
        .position(|major| *major == RECOMMENDED_MAJOR)
        .or_else(|| majors.len().checked_sub(1));
    DialogSpec {
        title: tr!("settings-java-download-title"),
        intro: Some(match source {
            JavaSource::Mojang => tr!("settings-java-download-intro-mojang"),
            JavaSource::OracleGraalVm => tr!("settings-java-download-intro-graalvm"),
        }),
        fields: vec![FieldSpec {
            label: tr!("settings-java-download-version"),
            help: None,
            placeholder: "",
            value: String::new(),
            kind: FieldKind::Pick { options, selected },
        }],
        parse: Rc::new(move |values| {
            let major = values[0]
                .trim()
                .strip_prefix("Java ")
                .and_then(|major| major.parse().ok())
                .ok_or_else(|| tr!("settings-java-download-pick").to_owned())?;
            Ok(LiveIntent::DownloadJava { source, major })
        }),
        reset: None,
    }
}

/// Asks before a Java the launcher downloaded is deleted.
fn confirm_remove(java: &JavaRow, handler: &LiveHandler, window: &mut Window, cx: &mut App) {
    let (title, home) = (java.title.clone(), java.home.clone());
    let handler = handler.clone();
    window.open_alert_dialog(cx, move |alert, _, _| {
        let (handler, home) = (handler.clone(), home.clone());
        alert
            .title(tr!("settings-java-remove-title", title = title.clone()))
            .description(tr!("settings-java-remove-body"))
            .ok_text(tr!("common-delete"))
            .ok_variant(gpui_component::button::ButtonVariant::Danger)
            .cancel_text(tr!("common-cancel"))
            .show_cancel(true)
            .on_ok(move |_, window, cx| {
                handler(LiveIntent::RemoveJava(home.clone()), window, cx);
                true
            })
    });
}

pub(super) fn java(view: &SettingsView, ctx: &SettingsCtx) -> AnyElement {
    let colors = ctx.colors;
    let handler = &ctx.handler;
    let list = if view.java.is_empty() {
        kit::empty(
            tr!("settings-java-none"),
            tr!("settings-java-none-help"),
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
                        tr!("settings-java-enable"),
                        send(
                            handler,
                            LiveIntent::SetJavaDisabled {
                                home: java.home.clone(),
                                disabled: enabled,
                            },
                        ),
                    );
                    // ia[settings]: Java：在访达中显示 | Java · 列表行 ⋯ 菜单 | 打开并选中它所在位置
                    let mut entries = vec![kit::MenuEntry::new(
                        crate::platform::reveal_label(),
                        send(handler, LiveIntent::Reveal(java.home.clone())),
                    )];
                    if java.removable {
                        let (row, handler) = (java.clone(), handler.clone());
                        // ia[settings]: Java：删除 | Java · 列表行 ⋯ 菜单「删除…」（只有启动器下载的 Java）→ 确认框 | 删除它的文件夹并重新检测；其它 Java 只能停用
                        entries.push(
                            kit::MenuEntry::new(tr!("settings-java-remove"), move |window, cx| {
                                confirm_remove(&row, &handler, window, cx);
                            })
                            .danger(),
                        );
                    }
                    let more = kit::more_menu(("settings-java-more", index), entries, colors);
                    kit::row(
                        if java.disabled {
                            tr!("settings-java-disabled", title = java.title.as_str())
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
        tr!("settings-java-roots"),
        Some(tr!("settings-java-roots-help").to_owned()),
        if roots.is_empty() {
            tr!("common-none").to_owned()
        } else {
            tr!("settings-java-roots-count", count = roots.len())
        },
        Some(edit("settings-java-roots-edit", handler, move || {
            DialogSpec {
                title: tr!("settings-java-roots"),
                intro: Some(tr!("settings-java-roots-intro")),
                fields: vec![FieldSpec {
                    label: tr!("settings-java-roots-field"),
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
    // ia[settings]: Java：下载来源 | Java · 分段「Mojang / Oracle GraalVM」 | 下载与启动时自动下载都从这里取；GraalVM 只有 Java 21 和 25，其它版本仍从 Mojang 下载
    let source_row = preference_row(
        "settings-java-source",
        tr!("settings-java-source"),
        Some(tr!("settings-java-source-help")),
        tr_all![
            "settings-java-source-mojang",
            "settings-java-source-graalvm"
        ],
        source_index(view.java_source),
        |index| LiveIntent::SetJavaSource(JavaSource::ALL[index]),
        ctx,
    );
    let auto_install = view.auto_install_java;
    let auto_row = row(
        "settings-java-auto-install",
        tr!("settings-java-auto-install"),
        Some(tr!("settings-java-auto-install-help").to_owned()),
        "",
        Some(
            // ia[settings]: Java：启动时自动下载 | Java · 开关 | 开启时，启动或安装时没有合适的 Java 就先下载游戏需要的版本，进度在动态；关闭后提示去安装
            kit::switch(
                "settings-java-auto-install-switch",
                auto_install,
                tr!("settings-java-auto-install"),
                send(handler, LiveIntent::SetAutoInstallJava(!auto_install)),
            )
            .into_any_element(),
        ),
        colors,
    );
    let download = {
        let source = view.java_source;
        let majors = offered(view, source);
        let handler = handler.clone();
        // ia[settings]: Java：下载 Java… | Java · 按键「下载 Java…」→ 弹窗选大版本 | 版本来自所选下载来源（Mojang 读运行时索引），默认 21；装好后出现在列表里，进度在动态
        kit::action(
            "settings-java-install",
            tr!("settings-java-install"),
            Some(crate::assets::UiIcon::Download),
            false,
            move |window, cx| {
                SettingsDialog::open(download_spec(source, &majors), handler.clone(), window, cx);
            },
        )
    };
    v_flex()
        .w_full()
        .gap_5()
        // ia[settings]: Java：检测与列表 | Java · 已发现的 Java 列表 | 列出检测到的 Java（版本、发行方、架构、路径），包括官方启动器的运行时与 PATH 中的 java；没有时提示添加
        .child(kit::section(tr!("settings-java-found"), colors, list))
        .child(
            h_flex()
                .gap_2()
                // ia[settings]: Java：重新检测 | Java · 按键「重新检测」 | 重新读取设置并检测
                .child(kit::action(
                    "settings-java-rescan",
                    tr!("settings-java-rescan"),
                    Some(crate::assets::UiIcon::Refresh),
                    false,
                    send(handler, LiveIntent::LoadSettings),
                ))
                .child(download)
                // ia[settings]: Java：添加 | Java · 按键「添加 Java…」→ 选 java 程序或 JDK 文件夹 | 加入列表
                .child(kit::action(
                    "settings-java-add",
                    tr!("settings-java-add"),
                    Some(crate::assets::UiIcon::Plus),
                    false,
                    send(handler, LiveIntent::AddJava),
                )),
        )
        .child(kit::list(vec![source_row, auto_row, roots_row], colors))
        .into_any_element()
}

#[cfg(test)]
mod tests;
