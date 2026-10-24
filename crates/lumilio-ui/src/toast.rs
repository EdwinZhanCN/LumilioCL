//! Messages about what just happened (design language §11): they float as
//! toasts and never take room in a page.
//!
//! Views of the main window [`post`] their [`Toast`]s, and the shell, which
//! is always drawn there, shows them on its next render whatever page is on
//! screen: a result that arrives after its page was left is still told.
//! Another window (the schematic studio) queues its own and [`flush`]es
//! them. A window without the framework `Root` (some tests) keeps them
//! queued, so they stay observable.

use gpui::prelude::*;
use gpui::{App, ClipboardItem, Global, SharedString, Window, div, px};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::notification::Notification;
use gpui_component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};

use crate::key::Key;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToastKind {
    /// Something started or is worth knowing; hides itself.
    Info,
    /// Something finished; hides itself.
    Success,
    /// Something did not work; stays until dismissed.
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Toast {
    pub kind: ToastKind,
    /// One plain sentence (design language §8).
    pub text: String,
    /// The raw cause, behind **技术详情**; never the headline.
    pub technical: Option<String>,
    /// Stays until dismissed even when it is not an error.
    pub sticky: bool,
    /// A page to open from the toast: its key label and address.
    pub link: Option<(String, String)>,
}

impl Toast {
    fn new(kind: ToastKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
            technical: None,
            sticky: kind == ToastKind::Error,
            link: None,
        }
    }

    pub fn info(text: impl Into<String>) -> Self {
        Self::new(ToastKind::Info, text)
    }

    pub fn success(text: impl Into<String>) -> Self {
        Self::new(ToastKind::Success, text)
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self::new(ToastKind::Error, text)
    }

    pub fn technical(mut self, detail: impl Into<String>) -> Self {
        self.technical = Some(detail.into());
        self
    }

    pub fn sticky(mut self) -> Self {
        self.sticky = true;
        self
    }

    /// Offers a page in the browser from the toast (a download page), in
    /// place of technical details.
    pub fn link(mut self, label: impl Into<String>, url: impl Into<String>) -> Self {
        self.link = Some((label.into(), url.into()));
        self
    }
}

/// The main window's toasts not shown yet.
#[derive(Default)]
pub struct Posted(Vec<Toast>);

impl Global for Posted {}

/// Queues a toast for the main window, from any of its views.
pub fn post(toast: Toast, cx: &mut App) {
    cx.default_global::<Posted>().0.push(toast);
}

/// Toasts posted for the main window and not shown yet.
pub fn posted(cx: &App) -> &[Toast] {
    cx.try_global::<Posted>().map_or(&[], |posted| &posted.0)
}

/// Shows the main window's posted toasts, oldest first, if the window can.
pub fn flush_posted(window: &mut Window, cx: &mut App) {
    // Only taken mutably when there is something to show: that tells the
    // shell, which observes them, to draw again.
    if posted(cx).is_empty() || !has_root(window) {
        return;
    }
    let mut queue = std::mem::take(&mut cx.global_mut::<Posted>().0);
    flush(&mut queue, window, cx);
}

/// Whether this window can show toasts and dialogs.
pub fn has_root(window: &Window) -> bool {
    matches!(window.root::<gpui_component::Root>(), Some(Some(_)))
}

/// Shows every queued toast, oldest first, if the window can.
pub fn flush(queue: &mut Vec<Toast>, window: &mut Window, cx: &mut App) {
    if queue.is_empty() || !has_root(window) {
        return;
    }
    for toast in queue.drain(..) {
        show(toast, window, cx);
    }
}

fn show(toast: Toast, window: &mut Window, cx: &mut App) {
    // What failed is also kept in the launcher log, with its raw cause.
    if toast.kind == ToastKind::Error {
        match &toast.technical {
            Some(cause) => {
                let said = toast.text.trim_end_matches(['。', '.']);
                log::error!("{said}: {cause}");
            }
            None => log::error!("{}", toast.text),
        }
    }
    let note = match toast.kind {
        ToastKind::Info => Notification::info(toast.text),
        ToastKind::Success => Notification::success(toast.text),
        ToastKind::Error => Notification::error(toast.text),
    };
    let note = match (toast.link, toast.technical) {
        (Some((label, url)), _) => note.action(move |_, _, _| {
            let (label, url) = (label.clone(), url.clone());
            crate::theme::clickable(
                // gpui-component's notification only accepts its own `Button`.
                Button::new("toast-link").label(label).ghost().small(),
                true,
            )
            .on_click(move |_, _, cx| cx.open_url(&url))
        }),
        (None, Some(detail)) => note.action(move |_, _, _| {
            let detail = detail.clone();
            crate::theme::clickable(
                // gpui-component's notification only accepts its own `Button`.
                Button::new("toast-technical")
                    .label(crate::tr!("common-technical-details"))
                    .ghost()
                    .small(),
                true,
            )
            .on_click(move |_, window, cx| technical_dialog(detail.clone(), window, cx))
        }),
        (None, None) => note.autohide(!toast.sticky),
    };
    window.push_notification(note, cx);
}

/// The raw cause, selectable and copyable, for people who need it (§8).
/// Puts text on the system clipboard.
pub fn copy_text(text: impl Into<String>, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(text.into()));
}

pub fn technical_dialog(detail: impl Into<SharedString>, window: &mut Window, cx: &mut App) {
    let detail: SharedString = detail.into();
    window.open_dialog(cx, move |dialog, _, cx| {
        let copy = detail.clone();
        let mono = cx.theme().mono_font_family.clone();
        let muted = cx.theme().muted_foreground;
        crate::theme::dialog(dialog, cx)
            .title(crate::tr!("common-technical-details"))
            .w(px(520.))
            .child(
                v_flex().child(
                    div()
                        .id("technical-detail")
                        .max_h(px(280.))
                        .overflow_y_scroll()
                        .font_family(mono)
                        .text_xs()
                        .text_color(muted)
                        .child(detail.clone()),
                ),
            )
            .footer(
                h_flex()
                    .w_full()
                    .justify_end()
                    .gap_2()
                    .child(crate::theme::clickable(
                        Key::new("technical-copy")
                            .label(crate::tr!("common-copy"))
                            .white()
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copy.to_string()))
                            })
                            .debug_selector(|| "technical-copy".into()),
                        true,
                    ))
                    .child(crate::theme::clickable(
                        Key::new("technical-close")
                            .label(crate::tr!("common-close"))
                            .primary()
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                        true,
                    )),
            )
    });
}

#[cfg(test)]
mod tests;
