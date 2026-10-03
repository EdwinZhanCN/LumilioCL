//! Messages about what just happened (design language §11): they float as
//! toasts and never take room in a page.
//!
//! Views queue [`Toast`]s from wherever a result arrives and show them on
//! their next render, where a window is at hand. A window without the
//! framework `Root` (some tests) keeps them queued, so they stay observable.

use crate::key::Key;
use gpui::{App, ClipboardItem, SharedString, Window, div, prelude::*, px};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::notification::Notification;
use gpui_component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};

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
}

impl Toast {
    fn new(kind: ToastKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
            technical: None,
            sticky: kind == ToastKind::Error,
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
    let note = match toast.kind {
        ToastKind::Info => Notification::info(toast.text),
        ToastKind::Success => Notification::success(toast.text),
        ToastKind::Error => Notification::error(toast.text),
    };
    let note = match toast.technical {
        Some(detail) => note.action(move |_, _, _| {
            let detail = detail.clone();
            crate::theme::clickable(
                // gpui-component's notification only accepts its own `Button`.
                Button::new("toast-technical")
                    .label("技术详情")
                    .ghost()
                    .small(),
                true,
            )
            .on_click(move |_, window, cx| technical_dialog(detail.clone(), window, cx))
        }),
        None => note.autohide(!toast.sticky),
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
            .title("技术详情")
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
                        Key::new("technical-copy").label("复制").white().on_click(
                            move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copy.to_string()))
                            },
                        ),
                        true,
                    ))
                    .child(crate::theme::clickable(
                        Key::new("technical-close")
                            .label("关闭")
                            .primary()
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                        true,
                    )),
            )
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_errors_stay_by_default() {
        assert!(!Toast::info("a").sticky);
        assert!(!Toast::success("a").sticky);
        assert!(Toast::error("a").sticky);
        assert!(Toast::info("a").sticky().sticky);
        assert_eq!(
            Toast::error("没装上").technical("io").technical.as_deref(),
            Some("io")
        );
    }
}
