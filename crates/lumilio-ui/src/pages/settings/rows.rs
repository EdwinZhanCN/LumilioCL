use super::super::live::LiveCtx;
use crate::kit;
use crate::live::{LiveHandler, LiveIntent, SettingsView};
use crate::settings_dialog::{DialogSpec, SettingsDialog};
use crate::theme::ShellColors;
use crate::tr;
use gpui::prelude::*;
use gpui::{AnyElement, App, IntoElement, SharedString, Window};
use gpui_component::h_flex;
use lumilio_core::Preferences;

pub(super) fn send(
    handler: &LiveHandler,
    intent: LiveIntent,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |window, cx| handler(intent.clone(), window, cx)
}

/// An [编辑] button that opens a dialog built when it is pressed.
pub(super) fn edit(
    id: &'static str,
    handler: &LiveHandler,
    spec: impl Fn() -> DialogSpec<LiveIntent> + 'static,
) -> AnyElement {
    let handler = handler.clone();
    kit::ghost(id, tr!("common-edit"), move |window, cx| {
        SettingsDialog::open(spec(), handler.clone(), window, cx);
    })
    .debug_selector(move || id.to_owned())
    .into_any_element()
}

pub(super) fn row(
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

pub(super) fn preference_row(
    id: &'static str,
    label: &'static str,
    help: Option<&'static str>,
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
    row(
        id,
        label,
        help.map(str::to_owned),
        "",
        Some(control),
        ctx.colors,
    )
}

pub(super) fn with_preferences(
    view: &SettingsView,
    change: impl FnOnce(&mut Preferences) + 'static,
) -> LiveIntent {
    let mut next = view.preferences.clone();
    change(&mut next);
    LiveIntent::SetPreferences(next)
}
