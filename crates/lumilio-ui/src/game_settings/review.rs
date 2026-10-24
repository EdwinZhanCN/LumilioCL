//! Changes made in the game to options its preset shares. None is shared or
//! dropped without the player deciding (plan D3): each can stay with this
//! game, go into the preset, or go back to the preset's value. Selected rows
//! are decided together (P-BULK).

use std::collections::BTreeSet;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{AnyElement, App, IntoElement, SharedString, Window, div, px};
use gpui_component::{Sizable as _, StyledExt as _, WindowExt as _, h_flex, v_flex};
use lumilio_core::{GameSettingsView, SettingResolution};

use super::labels::{setting_label, value_label};
use super::table::{OptionLine, Source, sort_lines};
use crate::controls::Checkbox;
use crate::key::Key;
use crate::theme::ShellColors;
use crate::{kit, theme, tr};

type Act<T> = Rc<dyn Fn(T, &mut Window, &mut App)>;

/// What the review asks of the page that holds its selection.
#[derive(Clone)]
pub struct ReviewActions {
    /// Selects or clears one row.
    pub toggle: Act<String>,
    /// Selects every row (`true`) or none.
    pub all: Act<bool>,
    pub resolve: Act<SettingResolution>,
}

/// The review block, or `None` when nothing waits for a decision.
pub fn review(
    view: &GameSettingsView,
    selected: &BTreeSet<String>,
    conflicts: &[String],
    disabled: bool,
    colors: ShellColors,
    actions: &ReviewActions,
) -> Option<AnyElement> {
    if view.pending.is_empty() {
        return None;
    }
    let mut lines: Vec<OptionLine> = view
        .pending
        .iter()
        .map(|change| OptionLine {
            key: change.setting.clone(),
            value: change.value.clone(),
            source: Source::Pending,
        })
        .collect();
    sort_lines(&mut lines);
    let total = lines.len();
    let chosen = lines
        .iter()
        .filter(|line| selected.contains(&line.key))
        .count();
    let all = chosen == total;
    let head = {
        let pick_all = actions.all.clone();
        let select_all = Checkbox::new("options-review-all")
            .checked(all)
            .disabled(disabled)
            .on_click(move |_, window, cx| pick_all(!all, window, cx));
        let mut head = h_flex()
            .w_full()
            .h(px(34.))
            .gap_3()
            .items_center()
            .child(select_all);
        if chosen == 0 {
            head = head.child(
                div()
                    .flex_1()
                    .text_sm()
                    .text_color(colors.muted)
                    .child(tr!("options-review-none-selected")),
            );
        } else {
            head = head
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .text_color(colors.foreground)
                        .child(tr!("options-review-selected", count = chosen)),
                )
                .children(resolutions(disabled, actions));
        }
        head
    };
    let rows: Vec<_> = lines
        .iter()
        .map(|line| {
            let change = view
                .pending
                .iter()
                .find(|change| change.setting == line.key)
                .expect("lines come from the pending changes");
            let on = selected.contains(&line.key);
            let toggle = actions.toggle.clone();
            let key = line.key.clone();
            let conflict = conflicts
                .contains(&line.key)
                .then(|| {
                    view.preset
                        .as_ref()
                        .and_then(|preset| preset.values.get(&line.key))
                })
                .flatten();
            h_flex()
                .id(SharedString::from(format!("options-review-{}", line.key)))
                .w_full()
                .gap_3()
                .py(px(9.))
                .items_center()
                .child(
                    Checkbox::new(SharedString::from(format!(
                        "options-review-check-{}",
                        line.key
                    )))
                    .checked(on)
                    .disabled(disabled)
                    .on_click(move |_, window, cx| toggle(key.clone(), window, cx)),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_sm()
                                .font_medium()
                                .text_color(colors.foreground)
                                .child(setting_label(&line.key)),
                        )
                        .children(conflict.map(|shared| {
                            div().text_xs().text_color(colors.danger).child(tr!(
                                "options-review-conflict",
                                value = value_label(&line.key, shared)
                            ))
                        })),
                )
                .child(
                    div()
                        .flex_none()
                        .font_family(theme::mono_font())
                        .text_xs()
                        .text_color(colors.muted)
                        .child(tr!(
                            "options-change",
                            before = value_label(&line.key, &change.before),
                            after = value_label(&line.key, &change.value)
                        )),
                )
        })
        .collect();
    let replace = (!conflicts.is_empty()).then(|| {
        let resolve = actions.resolve.clone();
        let count = conflicts.len();
        // ia[instance.settings]: 用这个游戏的值替换预设 | 选项 · 待决定列表冲突说明旁的「用这个游戏的值替换…」→ 警告弹窗 | 预设里同一项也被改过时，看过预设的新值、确认后才替换；其他游戏下次启动才收到
        h_flex()
            .w_full()
            .gap_3()
            .items_center()
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .text_color(colors.danger)
                    .child(tr!("options-review-conflict-help")),
            )
            .child(theme::clickable(
                Key::new("options-review-replace")
                    .label(tr!("options-review-replace"))
                    .white()
                    .small()
                    .disabled(disabled)
                    .debug_selector(|| "options-review-replace".into())
                    .on_click(move |_, window, cx| {
                        // Replacing what the preset now holds is confirmed
                        // with its consequence (design language §10).
                        let resolve = resolve.clone();
                        window.open_alert_dialog(cx, move |alert, _, _| {
                            let resolve = resolve.clone();
                            alert
                                .title(tr!("options-replace-title", count = count))
                                .description(tr!("options-replace-body"))
                                .ok_text(tr!("options-replace"))
                                .ok_variant(gpui_component::button::ButtonVariant::Danger)
                                .cancel_text(tr!("common-cancel"))
                                .show_cancel(true)
                                .on_ok(move |_, window, cx| {
                                    resolve(SettingResolution::ReplaceShared, window, cx);
                                    true
                                })
                        });
                    }),
                !disabled,
            ))
    });
    Some(
        v_flex()
            .w_full()
            .gap_3()
            .child(kit::section_label(
                tr!("options-review-title", count = total),
                colors,
            ))
            .child(head)
            .child(kit::list(rows, colors))
            .children(replace)
            .into_any_element(),
    )
}

fn resolutions(disabled: bool, actions: &ReviewActions) -> Vec<AnyElement> {
    // ia[instance.settings]: 处理游戏里改过的选项 | 选项 · 待决定列表勾选后：仅此游戏 / 更新预设 / 改回预设值 | 一起处理所选各项；更新预设前检查预设是否也改过，冲突时保留选择并标出预设的新值
    [
        (
            SettingResolution::Instance,
            tr!("options-keep-here"),
            tr!("options-keep-here-help"),
            "local",
        ),
        (
            SettingResolution::Shared,
            tr!("options-share"),
            tr!("options-share-help"),
            "shared",
        ),
        (
            SettingResolution::Restore,
            tr!("options-restore"),
            tr!("options-restore-help"),
            "restore",
        ),
    ]
    .into_iter()
    .map(|(resolution, label, help, slug)| {
        let resolve = actions.resolve.clone();
        theme::clickable(
            Key::new(SharedString::from(format!("options-resolve-{slug}")))
                .label(label)
                .tooltip(help)
                .white()
                .small()
                .disabled(disabled)
                .debug_selector(move || format!("options-resolve-{slug}"))
                .on_click(move |_, window, cx| resolve(resolution, window, cx)),
            !disabled,
        )
        .into_any_element()
    })
    .collect()
}
