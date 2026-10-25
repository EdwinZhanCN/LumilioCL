//! The Library's 预设 tab: the presets on the leading side, the chosen one's
//! detail on the trailing side, each scrolling on its own (the master–detail
//! of Accounts). The detail is the preset's own option table: what is changed
//! here reaches every game that uses it at that game's next launch.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{AnyElement, App, IntoElement, SharedString, Window, div, px};
use gpui_component::{StyledExt as _, WindowExt as _, h_flex, v_flex};
use lumilio_core::{GameSettingValue, GameSettingsView, SettingsPreset};

use super::page::{PRESET_GROUP, PRESETS_TAB};
use super::view::{LibraryCtx, page_actions, send, toolbar};
use crate::assets::UiIcon;
use crate::game_settings::{OnEdit, OnToggle, PresetIntent};
use crate::instance_detail::{ProblemAction, SETTINGS_OPTIONS};
use crate::kit::ViewIntent;
use crate::live::LiveIntent;
use crate::{game_settings, kit, theme, tr};

/// The preset the detail shows: the one last chosen, or the first.
fn shown_preset(presets: &[SettingsPreset], chosen: i64) -> Option<&SettingsPreset> {
    presets
        .iter()
        .find(|preset| preset.id == chosen)
        .or_else(|| presets.first())
}

fn users<'a>(ctx: &'a LibraryCtx, preset: &SettingsPreset) -> Vec<&'a GameSettingsView> {
    ctx.model
        .game_settings
        .iter()
        .filter(|view| view.preset.as_ref().is_some_and(|p| p.id == preset.id))
        .collect()
}

fn shared_count(preset: &SettingsPreset) -> usize {
    preset.values.len() - preset.excluded.len()
}

fn list_item(preset: &SettingsPreset, shown: bool, ctx: &LibraryCtx) -> gpui::Stateful<gpui::Div> {
    let colors = ctx.colors;
    let users = users(ctx, preset);
    let attention = users.iter().any(|view| game_settings::game_status(view).1);
    let emit = ctx.emit.clone();
    let id = usize::try_from(preset.id).unwrap_or(0);
    // ia[library]: 预设：查看 | 预设 tab 左侧列表的一行（名称、共享几项、几个游戏在用；有游戏待决定时亮灯） | 右侧显示这个预设共享的选项和使用它的游戏
    h_flex()
        .id(("preset-item", id))
        .debug_selector(move || format!("preset-item-{id}"))
        .w_full()
        .items_center()
        .gap_3()
        .px_3()
        .py(px(10.))
        .rounded_md()
        .cursor_pointer()
        .when(shown, |item| item.bg(colors.surface_subtle))
        .hover(|item| item.bg(colors.surface_subtle))
        .on_click(move |_, window, cx| emit(ViewIntent::Choose(PRESET_GROUP, id), window, cx))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(2.))
                .child(
                    div()
                        .text_sm()
                        .font_medium()
                        .text_color(colors.foreground)
                        .truncate()
                        .child(preset.name.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.muted)
                        .truncate()
                        .child(tr!(
                            "preset-list-detail",
                            options = shared_count(preset),
                            games = users.len()
                        )),
                ),
        )
        .children(attention.then(|| crate::controls::tone_led(kit::tone_warn())))
}

fn confirm_delete(
    preset: &SettingsPreset,
    ctx: &LibraryCtx,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let handler = ctx.handler.clone();
    let (id, name) = (preset.id, preset.name.clone());
    move |window: &mut Window, cx: &mut App| {
        let handler = handler.clone();
        let title = tr!("preset-delete-title", name = name.as_str());
        window.open_alert_dialog(cx, move |alert, _, _| {
            let handler = handler.clone();
            alert
                .title(title.clone())
                .description(tr!("preset-delete-body"))
                .ok_text(tr!("common-delete"))
                .ok_variant(gpui_component::button::ButtonVariant::Danger)
                .cancel_text(tr!("common-cancel"))
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    handler(LiveIntent::Preset(PresetIntent::Delete(id)), window, cx);
                    true
                })
        });
    }
}

fn detail_head(preset: &SettingsPreset, used: usize, ctx: &LibraryCtx) -> impl IntoElement {
    let colors = ctx.colors;
    let go = |intent: PresetIntent| send(&ctx.handler, LiveIntent::Preset(intent));
    // ia[library]: 预设：重命名 | 预设详情 ⋯ 菜单「重命名…」→ 弹窗 | 名称 1–80 个字、不与其他预设重名；结果 toast 手动关闭
    // ia[library]: 预设：复制 | 预设详情 ⋯ 菜单「复制…」→ 弹窗取名 | 新预设有自己的选项和共享范围，不带走使用它的游戏
    // ia[library]: 预设：删除 | 预设详情 ⋯ 菜单「删除…」→ 警告弹窗 | 只删预设，游戏自己的选项不变；还有游戏在用时不可选
    let menu = kit::more_menu(
        ("preset-more", usize::try_from(preset.id).unwrap_or(0)),
        vec![
            kit::MenuEntry::new(tr!("preset-rename"), go(PresetIntent::Rename(preset.id))),
            kit::MenuEntry::new(tr!("preset-copy"), go(PresetIntent::Copy(preset.id))),
            kit::MenuEntry::new(tr!("preset-delete"), confirm_delete(preset, ctx))
                .danger()
                .disabled(used > 0),
        ],
        colors,
    );
    // ia[library]: 预设：共享范围 | 预设详情头部「共享范围…」→ 弹窗：按组和逐项勾选 | 取消勾选的选项值仍留在预设里，可再勾回；使用它的游戏在运行或有待决定的修改时不能改
    let scope = kit::action(
        ("preset-scope", usize::try_from(preset.id).unwrap_or(0)),
        tr!("preset-scope"),
        None,
        false,
        go(PresetIntent::Scope(preset.id)),
    )
    .debug_selector(|| "preset-scope".into());
    h_flex()
        .w_full()
        .items_center()
        .gap_4()
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(
                    div()
                        .text_xl()
                        .font_semibold()
                        .text_color(colors.foreground)
                        .truncate()
                        .child(preset.name.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.muted)
                        .truncate()
                        .child(tr!(
                            "preset-detail-facts",
                            options = shared_count(preset),
                            games = used
                        )),
                ),
        )
        .child(scope)
        .child(menu)
}

/// The games that follow the preset, each with how it stands.
fn games(users: &[&GameSettingsView], ctx: &LibraryCtx) -> AnyElement {
    let colors = ctx.colors;
    let title = h_flex()
        .gap_1()
        .items_center()
        .child(kit::section_label(tr!("preset-games"), colors))
        .child(kit::info("preset-games-info", tr!("preset-games-help")));
    if users.is_empty() {
        return v_flex()
            .w_full()
            .gap_3()
            .child(title)
            .child(
                div()
                    .text_sm()
                    .text_color(colors.muted)
                    .child(tr!("preset-games-none")),
            )
            .into_any_element();
    }
    let rows = users
        .iter()
        .map(|view| {
            let name = ctx
                .model
                .library
                .iter()
                .find(|card| card.id == view.instance)
                .map_or_else(|| view.instance.clone(), |card| card.name.clone());
            let (status, attention) = game_settings::game_status(view);
            let open = send(
                &ctx.handler,
                LiveIntent::Resolve(
                    view.instance.clone(),
                    ProblemAction::Settings(SETTINGS_OPTIONS),
                ),
            );
            let selector = format!("preset-game-{}", view.instance);
            // ia[library]: 预设：打开使用它的游戏 | 预设详情「使用它的游戏」每行的「打开」 | 打开该游戏的 设置 › 选项（有待决定的修改时在最上面），不启动游戏
            let key = theme::clickable(
                kit::ghost(
                    SharedString::from(format!("preset-game-open-{}", view.instance)),
                    tr!("preset-game-open"),
                    move |window, cx| open(window, cx),
                )
                .debug_selector(move || selector.clone()),
                true,
            );
            h_flex()
                .w_full()
                .items_center()
                .gap_3()
                .py(px(9.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_sm()
                        .font_medium()
                        .text_color(colors.foreground)
                        .child(name),
                )
                .child(if attention {
                    kit::chip(status, Some(kit::tone_warn()), colors).into_any_element()
                } else {
                    div()
                        .text_xs()
                        .text_color(colors.muted)
                        .child(status)
                        .into_any_element()
                })
                .child(key)
        })
        .collect();
    v_flex()
        .w_full()
        .gap_3()
        .child(title)
        .child(kit::panel_list(rows, colors))
        .into_any_element()
}

fn detail(preset: &SettingsPreset, ctx: &LibraryCtx) -> AnyElement {
    let colors = ctx.colors;
    let users = users(ctx, preset);
    let needle = ctx.filter.trim().to_lowercase();
    let lines: Vec<_> = game_settings::preset_lines(preset)
        .into_iter()
        .filter(|line| game_settings::matches(line, &needle))
        .collect();
    let id = preset.id;
    let on_edit: OnEdit = {
        let handler = ctx.handler.clone();
        Rc::new(move |key, window, cx| {
            handler(
                LiveIntent::Preset(PresetIntent::Edit { preset: id, key }),
                window,
                cx,
            )
        })
    };
    let on_toggle: OnToggle = {
        let handler = ctx.handler.clone();
        Rc::new(move |key, on, window, cx| {
            handler(
                LiveIntent::Preset(PresetIntent::Set {
                    preset: id,
                    key,
                    before: GameSettingValue::Boolean(!on),
                    value: GameSettingValue::Boolean(on),
                }),
                window,
                cx,
            )
        })
    };
    let left_out = preset.excluded.len();
    // ia[library]: 预设：修改选项 | 预设详情的选项表：开关当场切换，其他选项「编辑」→ 弹窗（滑块和数值 / 分段 / 可搜索列表） | 存进预设，使用它的游戏下次启动时收到；游戏里改过还没决定的同一项，决定时会看到冲突
    let options =
        game_settings::table("preset-option", &lines, false, colors, &on_edit, &on_toggle);
    div()
        .id("preset-detail")
        .debug_selector(|| "preset-detail".into())
        .size_full()
        .min_h_0()
        .overflow_y_scroll()
        .child(
            v_flex()
                .w_full()
                .gap_6()
                .pb(theme::BOTTOM_SAFE_AREA)
                .child(detail_head(preset, users.len(), ctx))
                .child(games(&users, ctx))
                .child(options)
                .children((left_out > 0 && needle.is_empty()).then(|| {
                    div()
                        .text_xs()
                        .text_color(colors.muted)
                        .child(tr!("preset-left-out", count = left_out))
                })),
        )
        .into_any_element()
}

pub(super) fn presets(ctx: &LibraryCtx) -> AnyElement {
    let colors = ctx.colors;
    let model = ctx.model;
    let new = send(&ctx.handler, LiveIntent::Preset(PresetIntent::New));
    // ia[library]: 预设：新建 | 预设 tab 的次要页面操作「新建预设…」→ 弹窗：名称、来源游戏（可搜索）、共享的选项组 | 只读来源游戏的选项文件，不改它；默认不共享画面与性能；建好后在列表中选中，不让任何游戏自动使用
    let create = kit::action(
        "preset-new",
        tr!("preset-new"),
        Some(UiIcon::Plus),
        false,
        new,
    )
    .debug_selector(|| "preset-new".into());
    let head = v_flex()
        .w_full()
        .gap_5()
        .child(kit::header(
            tr!("route-library"),
            tr!("preset-count", count = model.settings_presets.len()),
            page_actions(create, ctx).render(colors),
            colors,
        ))
        .child(toolbar(PRESETS_TAB, ctx));
    let body = if let Some(error) = model.presets_error.as_ref() {
        kit::scroll_body(
            "presets-error",
            v_flex()
                .items_center()
                .gap_3()
                .child(kit::empty(tr!("preset-load-failed"), "", colors))
                .child(
                    h_flex()
                        .gap_2()
                        .child(kit::action(
                            "presets-retry",
                            tr!("common-retry"),
                            None,
                            false,
                            send(&ctx.handler, LiveIntent::Refresh),
                        ))
                        .child(kit::technical("presets-technical", error.clone())),
                ),
        )
        .into_any_element()
    } else if let Some(shown) = shown_preset(&model.settings_presets, ctx.view.preset) {
        let list = v_flex()
            .id("preset-list")
            .w(px(240.))
            .flex_none()
            .h_full()
            .min_h_0()
            .overflow_y_scroll()
            .gap_1()
            .pb(theme::BOTTOM_SAFE_AREA)
            .children(
                model
                    .settings_presets
                    .iter()
                    .map(|preset| list_item(preset, preset.id == shown.id, ctx)),
            );
        let pane = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .min_h_0()
            .child(detail(shown, ctx));
        kit::pane_body(
            h_flex()
                .w_full()
                .h_full()
                .min_h_0()
                .items_start()
                .gap_6()
                .child(list)
                .child(pane),
        )
        .into_any_element()
    } else {
        let new = send(&ctx.handler, LiveIntent::Preset(PresetIntent::New));
        kit::scroll_body(
            "presets-empty",
            v_flex()
                .items_center()
                .gap_3()
                .child(kit::empty(
                    tr!("preset-empty"),
                    tr!("preset-empty-help"),
                    colors,
                ))
                .child(kit::action(
                    "preset-empty-new",
                    tr!("preset-new"),
                    Some(UiIcon::Plus),
                    true,
                    new,
                )),
        )
        .into_any_element()
    };
    kit::fixed_page("live-presets", head, body).into_any_element()
}
