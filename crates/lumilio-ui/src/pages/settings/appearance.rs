//! The 外观 tab: which theme the launcher wears in the light and the dark,
//! which fonts it uses and how large it is. Long lists (themes, installed
//! fonts) are picked in a dialog with a searchable list (design language §10).

use super::super::live::LiveCtx;
use super::rows::{preference_row, row, send, with_preferences};
use crate::kit;
use crate::live::{LiveHandler, LiveIntent, SettingsView};
use crate::settings_dialog::{DialogSpec, FieldKind, FieldSpec, SettingsDialog};
use crate::theme::{ALUMINIUM, NIGHT, Tone};
use crate::{tr, tr_all};
use gpui::prelude::*;
use gpui::{AnyElement, App, IntoElement};
use lumilio_core::{Appearance, LookPreferences};
use std::rc::Rc;

/// The scales offered, in percent; the labels are `settings-scale-*`.
const SCALES: [u16; 4] = [85, 100, 115, 130];

pub(super) fn appearance(view: &SettingsView, ctx: &LiveCtx) -> AnyElement {
    let colors = ctx.colors;
    let prefs = &view.preferences;
    let look = &prefs.look;
    let mode = match prefs.appearance {
        Appearance::System => 0,
        Appearance::Light => 1,
        Appearance::Dark => 2,
    };
    let scale = SCALES
        .iter()
        .position(|percent| *percent == look.scale_percent())
        .unwrap_or(1);
    let mut rows = vec![
        // ia[settings]: 明暗 | 外观 · 分段：跟随系统 / 浅色 / 深色 | 立即生效并保存
        preference_row(
            "settings-appearance",
            tr!("settings-appearance-mode"),
            None,
            tr_all![
                "settings-follow-system",
                "settings-appearance-light",
                "settings-appearance-dark",
            ],
            mode,
            {
                let view = view.clone();
                move |index| {
                    with_preferences(&view, move |prefs| {
                        prefs.appearance =
                            [Appearance::System, Appearance::Light, Appearance::Dark][index.min(2)];
                    })
                }
            },
            ctx,
        ),
        // ia[settings]: 浅色主题 | 外观 · 值 + 更改 → 对话框里可搜索的主题列表 | 立即切换并保存；“恢复默认”回到内置的铝
        theme_row(
            "settings-theme-light",
            tr!("settings-theme-light"),
            Tone::Light,
            view,
            ctx,
        ),
        // ia[settings]: 深色主题 | 外观 · 值 + 更改 → 对话框里可搜索的主题列表 | 立即切换并保存；“恢复默认”回到内置的夜
        theme_row(
            "settings-theme-dark",
            tr!("settings-theme-dark"),
            Tone::Dark,
            view,
            ctx,
        ),
    ];
    if let Some(missing) = ctx
        .look
        .map(|look| look.theme.missing.len())
        .filter(|n| *n > 0)
    {
        rows.push(note(
            "settings-theme-missing-note",
            tr!("settings-theme-missing", count = missing),
            ctx,
        ));
    }
    let themes = view.data_dir.join("themes");
    rows.push(local_row(&themes, ctx));
    for (index, rejected) in ctx
        .catalog
        .iter()
        .flat_map(|catalog| &catalog.rejected)
        .enumerate()
    {
        let file = rejected
            .file
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        rows.push(note(
            ("settings-theme-rejected", index),
            tr!(
                "settings-theme-rejected",
                file = file,
                reason = rejected.reason.clone()
            ),
            ctx,
        ));
    }
    rows.extend([
        // ia[settings]: 界面字体 | 外观 · 值 + 更改 → 对话框里可搜索的已装字体 | 立即生效并保存；“恢复默认”回到 Space Grotesk
        font_row(
            &FontSlot {
                id: "settings-font-sans",
                edit_id: "settings-font-sans-edit",
                label: tr!("settings-font-sans"),
                help: None,
                chosen: look.sans_font.as_deref(),
                set: |look, family| look.sans_font = family,
            },
            view,
            ctx,
        ),
        // ia[settings]: 等宽字体 | 外观 · 值 + 更改 → 对话框里可搜索的已装字体 | 版本号、数值和游戏日志跟着变；“恢复默认”回到 JetBrains Mono
        font_row(
            &FontSlot {
                id: "settings-font-mono",
                edit_id: "settings-font-mono-edit",
                label: tr!("settings-font-mono"),
                help: Some(tr!("settings-font-mono-help")),
                chosen: look.mono_font.as_deref(),
                set: |look, family| look.mono_font = family,
            },
            view,
            ctx,
        ),
        // ia[settings]: 中文字体 | 外观 · 值 + 更改 → 对话框里可搜索的已装字体 | 汉字优先用它；没装就回退系统中文字体并在这里说明，不会用拉丁字体画汉字
        font_row(
            &FontSlot {
                id: "settings-font-cjk",
                edit_id: "settings-font-cjk-edit",
                label: tr!("settings-font-cjk"),
                help: Some(tr!("settings-font-cjk-help")),
                chosen: look.cjk_font.as_deref(),
                set: |look, family| look.cjk_font = family,
            },
            view,
            ctx,
        ),
    ]);
    if ctx.look.is_some_and(|look| look.cjk_missing) {
        rows.push(note(
            "settings-font-cjk-missing",
            tr!("settings-font-cjk-missing").to_owned(),
            ctx,
        ));
    }
    // ia[settings]: 界面缩放 | 外观 · 分段：85% / 100% / 115% / 130% | 文字和间距一起缩放，立即生效并保存；范围有上下限，防止破版
    rows.push(preference_row(
        "settings-scale",
        tr!("settings-scale"),
        Some(tr!("settings-scale-help")),
        tr_all![
            "settings-scale-85",
            "settings-scale-100",
            "settings-scale-115",
            "settings-scale-130",
        ],
        scale,
        {
            let view = view.clone();
            move |index| {
                with_preferences(&view, move |prefs| {
                    let percent = SCALES[index.min(SCALES.len() - 1)];
                    prefs.look.font_scale = (percent != 100).then_some(percent);
                })
            }
        },
        ctx,
    ));
    rows.push(wallpaper_row(view, ctx));
    if ctx.wallpaper.is_some_and(|wallpaper| wallpaper.missing) {
        rows.push(note(
            "settings-wallpaper-missing",
            tr!("settings-wallpaper-missing").to_owned(),
            ctx,
        ));
    }
    kit::list(rows, colors).into_any_element()
}

fn wallpaper_row(view: &SettingsView, ctx: &LiveCtx) -> gpui::Stateful<gpui::Div> {
    let chosen = view.preferences.look.wallpaper.is_some();
    let handler = ctx.handler.clone();
    let mut control = gpui_component::h_flex().gap_2().child(
        // ia[settings]: 主页壁纸 | 外观 · 按键「选择图片…」→ 系统文件选择器 | 图片缩小后存进启动器文件夹，主页换成这张图，底部仍溶解进页面；不是图片就提示，当前壁纸不变
        kit::ghost(
            "settings-wallpaper-choose",
            tr!("settings-wallpaper-choose"),
            {
                let handler = handler.clone();
                move |window, cx| {
                    let picked = crate::platform::pick_path(
                        cx,
                        true,
                        false,
                        tr!("settings-wallpaper-prompt"),
                    );
                    let (window, handler) = (window.window_handle(), handler.clone());
                    cx.spawn(async move |cx| {
                        if let Some(path) = picked.await {
                            let _ = cx.update_window(window, |_, window, cx| {
                                handler(LiveIntent::SetWallpaper(path), window, cx)
                            });
                        }
                    })
                    .detach();
                }
            },
        ),
    );
    if chosen {
        control = control.child(
            // ia[settings]: 恢复默认壁纸 | 外观 · 按键 | 主页回到默认的像素世界，存下的图片一并删除
            kit::ghost(
                "settings-wallpaper-reset",
                tr!("common-restore-defaults"),
                send(
                    &handler,
                    with_preferences(view, |prefs| prefs.look.wallpaper = None),
                ),
            ),
        );
    }
    let value = if chosen {
        tr!("settings-wallpaper-chosen")
    } else {
        tr!("settings-wallpaper-default")
    };
    row(
        "settings-wallpaper",
        tr!("settings-wallpaper"),
        Some(tr!("settings-wallpaper-help").to_owned()),
        value,
        Some(control.into_any_element()),
        ctx.colors,
    )
}

/// A line of explanation under the rows it belongs to.
fn note(id: impl Into<gpui::ElementId>, text: String, ctx: &LiveCtx) -> gpui::Stateful<gpui::Div> {
    kit::value_row(id, "", None, text, None, ctx.colors)
}

/// A button that opens a dialog whose fields need the app (installed fonts).
fn pick_button(
    id: &'static str,
    handler: &LiveHandler,
    spec: impl Fn(&App) -> DialogSpec<LiveIntent> + 'static,
) -> AnyElement {
    let handler = handler.clone();
    kit::ghost(id, tr!("common-edit"), move |window, cx| {
        SettingsDialog::open(spec(cx), handler.clone(), window, cx);
    })
    .debug_selector(move || id.to_owned())
    .into_any_element()
}

fn theme_row(
    id: &'static str,
    label: &'static str,
    tone: Tone,
    view: &SettingsView,
    ctx: &LiveCtx,
) -> gpui::Stateful<gpui::Div> {
    let builtin = if tone.is_dark() { NIGHT } else { ALUMINIUM };
    let chosen = if tone.is_dark() {
        view.preferences.look.dark_theme.as_deref()
    } else {
        view.preferences.look.light_theme.as_deref()
    };
    let names: Vec<String> = ctx
        .catalog
        .map(|catalog| {
            catalog
                .of_tone(tone)
                .map(|entry| entry.spec.name.clone())
                .collect()
        })
        .unwrap_or_else(|| vec![builtin.to_owned()]);
    // A saved name that no longer resolves shows as the built-in one, which is
    // what is drawn.
    let shown = chosen
        .filter(|name| names.iter().any(|known| known == name))
        .unwrap_or(builtin)
        .to_owned();
    let view = view.clone();
    let edit_id = if tone.is_dark() {
        "settings-theme-dark-edit"
    } else {
        "settings-theme-light-edit"
    };
    let edit = pick_button(edit_id, &ctx.handler, move |_| {
        let selected = names.iter().position(|name| *name == shown);
        let apply = {
            let view = view.clone();
            Rc::new(move |values: &[String]| {
                let name = values.first().cloned().unwrap_or_default();
                Ok(with_preferences(&view, move |prefs| {
                    let slot = if tone.is_dark() {
                        &mut prefs.look.dark_theme
                    } else {
                        &mut prefs.look.light_theme
                    };
                    *slot = (!name.is_empty() && name != builtin).then_some(name);
                }))
            })
        };
        DialogSpec {
            title: label,
            intro: Some(tr!("settings-theme-pick-help")),
            fields: vec![FieldSpec {
                label,
                help: None,
                placeholder: "",
                value: String::new(),
                kind: FieldKind::Pick {
                    options: names.clone(),
                    selected,
                },
            }],
            parse: apply,
            reset: Some(with_preferences(&view, move |prefs| {
                if tone.is_dark() {
                    prefs.look.dark_theme = None;
                } else {
                    prefs.look.light_theme = None;
                }
            })),
        }
    });
    row(id, label, None, "", Some(edit), ctx.colors)
}

fn local_row(themes: &std::path::Path, ctx: &LiveCtx) -> gpui::Stateful<gpui::Div> {
    let handler = ctx.handler.clone();
    let folder = themes.to_owned();
    let control = gpui_component::h_flex()
        .gap_2()
        // ia[settings]: 重新读取本地主题 | 外观 · 按键 | 重新扫描 themes 文件夹；能用的出现在上面的列表里，不能用的在下面说明原因
        .child(kit::ghost(
            "settings-theme-reload",
            tr!("settings-theme-reload"),
            {
                let handler = handler.clone();
                move |window, cx| handler(LiveIntent::ReloadThemes, window, cx)
            },
        ))
        // ia[settings]: 打开主题文件夹 | 外观 · 按键 | 在系统文件管理器里显示 themes 文件夹，没有就先建好
        .child(kit::ghost(
            "settings-theme-folder",
            crate::platform::reveal_label(),
            move |_, cx| {
                // A few bytes of directory creation; no UI wait worth a thread.
                let _ = std::fs::create_dir_all(&folder);
                crate::platform::reveal(&folder, cx);
            },
        ))
        .into_any_element();
    row(
        "settings-theme-local",
        tr!("settings-theme-local"),
        Some(tr!("settings-theme-local-help").to_owned()),
        themes.display().to_string(),
        Some(control),
        ctx.colors,
    )
}

/// What differs between the font rows.
#[derive(Clone, Copy)]
struct FontSlot<'a> {
    id: &'static str,
    edit_id: &'static str,
    label: &'static str,
    help: Option<&'static str>,
    chosen: Option<&'a str>,
    set: fn(&mut LookPreferences, Option<String>),
}

/// One font family row: the chosen family or 默认, and a searchable list of
/// the installed ones.
fn font_row(slot: &FontSlot, view: &SettingsView, ctx: &LiveCtx) -> gpui::Stateful<gpui::Div> {
    let FontSlot {
        id,
        edit_id,
        label,
        help,
        chosen,
        set,
    } = *slot;
    let shown = chosen.map_or_else(|| tr!("settings-font-default").to_owned(), str::to_owned);
    let current = chosen.map(str::to_owned);
    let view = view.clone();
    let edit = pick_button(edit_id, &ctx.handler, move |cx| {
        let default = tr!("settings-font-default").to_owned();
        let mut options = vec![default.clone()];
        options.extend(installed_families(cx));
        let selected = Some(
            current
                .as_ref()
                .and_then(|family| options.iter().position(|option| option == family))
                .unwrap_or(0),
        );
        let apply = {
            let view = view.clone();
            Rc::new(move |values: &[String]| {
                let family = values.first().cloned().unwrap_or_default();
                let family = (!family.is_empty() && family != default).then_some(family);
                Ok(with_preferences(&view, move |prefs| {
                    set(&mut prefs.look, family)
                }))
            })
        };
        DialogSpec {
            title: label,
            intro: help,
            fields: vec![FieldSpec {
                label,
                help: None,
                placeholder: "",
                value: String::new(),
                kind: FieldKind::Pick { options, selected },
            }],
            parse: apply,
            reset: Some(with_preferences(&view, move |prefs| {
                set(&mut prefs.look, None)
            })),
        }
    });
    row(
        id,
        label,
        help.map(str::to_owned),
        shown,
        Some(edit),
        ctx.colors,
    )
}

/// The installed font families, sorted, without the system's hidden ones.
fn installed_families(cx: &App) -> Vec<String> {
    let mut names: Vec<String> = cx
        .text_system()
        .all_font_names()
        .into_iter()
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup();
    names
}
