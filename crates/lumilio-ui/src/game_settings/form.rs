//! What the option and preset dialogs share (design language §10): the
//! footer, the disabled state while a write runs, and the result. A finished
//! write closes the dialog; a failure keeps the draft open. Results are
//! persistent toasts that the person closes (Project Maintainer, 2026-10-12).

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{App, Entity, Render, WeakEntity, Window, div, px};
use gpui_component::{WindowExt as _, h_flex};

use crate::key::Key;
use crate::toast::Toast;
use crate::{theme, tr};

/// Where a dialog hands its draft: the application runs the write and answers
/// the dialog (by its handle) with `finished`.
pub type Submit<T, F> = Rc<dyn Fn(T, WeakEntity<F>, &mut Window, &mut App)>;

/// A dialog's body and its commit.
pub trait Form: Render + Sized {
    /// The commit key's label: the result, such as "创建" or "保存".
    fn commit_label(&self) -> &'static str;
    /// Whether the draft can be committed now.
    fn ready(&self, cx: &App) -> bool;
    /// Hands the draft to the application; [`Sending::begin`] has run.
    fn commit(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>);
    fn sending(&mut self) -> &mut Sending;
    fn sending_ref(&self) -> &Sending;
    /// A quiet action on the footer's leading side, such as "跟随预设".
    fn aside(&self, _: &Entity<Self>, _: &App) -> Option<Key> {
        None
    }
}

/// Whether a write is under way, and results waiting for the next render.
#[derive(Default)]
pub struct Sending {
    pub busy: bool,
    toasts: Vec<Toast>,
}

impl Sending {
    /// Starts a write once; `false` while one is already running.
    pub fn begin(&mut self) -> bool {
        !std::mem::replace(&mut self.busy, true)
    }

    /// Shows queued results; call from the dialog's render.
    pub fn flush(&mut self, window: &mut Window, cx: &mut App) {
        crate::toast::flush(&mut self.toasts, window, cx);
    }

    /// A write finished. Success closes the dialog with its notice; a failure
    /// keeps the draft and says why.
    pub fn finish(&mut self, result: Result<String, String>, window: &mut Window, cx: &mut App) {
        self.busy = false;
        match result {
            Ok(notice) => {
                if !notice.is_empty() {
                    self.toasts.push(Toast::success(notice).sticky());
                }
                self.flush(window, cx);
                if crate::toast::has_root(window) {
                    window.close_dialog(cx);
                }
            }
            Err(detail) => self.toasts.push(Toast::error(detail).sticky()),
        }
    }
}

/// What the commit key does: hands a ready draft over once; a second press
/// while it is written does nothing.
pub fn submit<F: Form>(form: &mut F, window: &mut Window, cx: &mut gpui::Context<F>) {
    if form.ready(cx) && form.sending().begin() {
        form.commit(window, cx);
        cx.notify();
    }
}

/// Opens `form` as a dialog titled `title`, with Cancel and its commit.
pub fn open<F: Form>(
    form: Entity<F>,
    title: impl Into<gpui::SharedString>,
    width: f32,
    window: &mut Window,
    cx: &mut App,
) {
    let title = title.into();
    window.open_dialog(cx, move |dialog, _, cx| {
        let state = form.read(cx);
        let busy = state.sending_ref().busy;
        let ready = state.ready(cx);
        let aside = state.aside(&form, cx);
        let label = state.commit_label();
        let weak = form.downgrade();
        let commit = theme::clickable(
            Key::new("form-commit")
                .label(label)
                .primary()
                // While it runs the key keeps its colour and turns inert.
                .disabled(!ready && !busy)
                .loading(busy)
                .debug_selector(|| "form-commit".into())
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |form, cx| submit(form, window, cx));
                }),
            ready && !busy,
        );
        let cancel = theme::clickable(
            Key::new("form-cancel")
                .label(tr!("common-cancel"))
                .white()
                .disabled(busy)
                .debug_selector(|| "form-cancel".into())
                .on_click(|_, window, cx| window.close_dialog(cx)),
            !busy,
        );
        theme::dialog(dialog, cx)
            .title(title.clone())
            .w(px(width))
            .child(form.clone())
            .footer(
                h_flex()
                    .w_full()
                    .gap_2()
                    .children(aside)
                    .child(div().flex_1())
                    .child(cancel)
                    .child(commit),
            )
    });
}
