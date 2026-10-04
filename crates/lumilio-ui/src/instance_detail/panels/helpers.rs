use super::super::{InstanceDetailView, Section};
use super::data::{Confirm, Loaded};
use crate::assets::UiIcon;
use crate::theme::ShellColors;
use crate::{kit, live};
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Window};
use gpui_component::{h_flex, v_flex};

pub(in super::super) fn clock(seconds: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    live::relative_time(seconds, now)
}

pub(super) type Act =
    dyn Fn(&mut InstanceDetailView, &mut Window, &mut Context<InstanceDetailView>);

/// A click handler that updates this view (weakly, so it never keeps it alive).
pub(in super::super) fn act(
    cx: &Context<InstanceDetailView>,
    run: impl Fn(&mut InstanceDetailView, &mut Window, &mut Context<InstanceDetailView>) + 'static,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let view = cx.entity().downgrade();
    let run: Box<Act> = Box::new(run);
    move |window, app| {
        if let Some(view) = view.upgrade() {
            view.update(app, |view, cx| run(view, window, cx));
        }
    }
}

pub(in super::super) fn act_index(
    cx: &Context<InstanceDetailView>,
    run: impl Fn(&mut InstanceDetailView, usize, &mut Window, &mut Context<InstanceDetailView>)
    + 'static,
) -> impl Fn(usize, &mut Window, &mut App) + 'static {
    let view = cx.entity().downgrade();
    move |index, window, app| {
        if let Some(view) = view.upgrade() {
            view.update(app, |view, cx| run(view, index, window, cx));
        }
    }
}

impl InstanceDetailView {
    /// A load error or a loading note instead of a body.
    pub(in super::super) fn status<T>(
        &self,
        data: &Loaded<T>,
        colors: ShellColors,
        retry: Section,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        match data {
            None => Some(kit::empty("正在读取…", "", colors).into_any_element()),
            Some(Err(detail)) => Some(
                v_flex()
                    .gap_3()
                    .child(kit::empty("没有读到这部分内容", "可以重试", colors))
                    .child(
                        h_flex()
                            .justify_center()
                            .gap_2()
                            .child(kit::action(
                                ("instance-section-retry", 0usize),
                                "重试",
                                Some(UiIcon::Refresh),
                                false,
                                act(cx, move |view, window, cx| view.request(retry, window, cx)),
                            ))
                            .child(kit::technical("instance-section-technical", detail.clone())),
                    )
                    .into_any_element(),
            ),
            Some(Ok(_)) => None,
        }
    }

    /// A button that asks first, in a dialog that says what goes away.
    pub(super) fn asking(
        &self,
        id: (&'static str, usize),
        label: &'static str,
        confirm: Confirm,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        kit::ghost(
            id,
            label,
            act(cx, move |view, window, cx| {
                view.ask_confirm(confirm.clone(), window, cx)
            }),
        )
        .disabled(self.busy)
        .debug_selector(move || format!("{}-{}", id.0, id.1))
        .into_any_element()
    }
}
