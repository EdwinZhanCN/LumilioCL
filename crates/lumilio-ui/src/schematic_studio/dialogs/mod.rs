//! The studio's operation dialogs (design language §10 dialog shape): fields
//! stacked with their labels above, 取消 and the commit key in the footer.
//! While the write runs the dialog stays open and disabled; a failure stays
//! in the dialog as one sentence under the fields; success closes it.

mod convert;
mod metadata;
mod replace;

pub(crate) use convert::ConvertDialog;
#[cfg(test)]
pub(crate) use convert::Group;
#[cfg(test)]
pub(crate) use convert::Planned;
#[cfg(test)]
pub(crate) use convert::detail as loss_words;
pub(crate) use metadata::MetadataDialog;
#[cfg(test)]
pub(crate) use replace::BlockChoice;
#[cfg(test)]
pub(crate) use replace::Preview;
pub(crate) use replace::{Opening, ReplaceDialog};

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, SharedString, WeakEntity, Window, div, px};
use gpui_component::{StyledExt as _, WindowExt as _, h_flex, v_flex};

use crate::key::Key;
use crate::kit;
use crate::theme::{self, ShellColors};
use crate::tr;

const DIALOG_WIDTH: f32 = 480.;

/// The open operation dialog, so answers can reach it (and tests can read it).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum OpenDialog {
    Replace(WeakEntity<ReplaceDialog>),
    Convert(WeakEntity<ConvertDialog>),
    Metadata(WeakEntity<MetadataDialog>),
}

/// Where a dialog's write stands. The studio moves it; the dialog draws it.
#[derive(Default)]
pub(crate) struct Submission {
    pub(crate) busy: bool,
    pub(crate) error: Option<(&'static str, Option<String>)>,
}

impl Submission {
    pub(crate) fn start(&mut self, cx: &mut Context<Self>) {
        self.busy = true;
        self.error = None;
        cx.notify();
    }

    pub(crate) fn fail(
        &mut self,
        sentence: &'static str,
        detail: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.busy = false;
        self.error = Some((sentence, detail));
        cx.notify();
    }

    pub(crate) fn finish(&mut self, cx: &mut Context<Self>) {
        self.busy = false;
        cx.notify();
    }

    /// A field changed: an old failure no longer describes the draft.
    pub(crate) fn clear(&mut self, cx: &mut Context<Self>) {
        if self.error.is_some() && !self.busy {
            self.error = None;
            cx.notify();
        }
    }
}

/// What the dialog frame needs from a form.
pub(crate) trait DialogForm: Render + Sized {
    fn status(&self) -> &Entity<Submission>;
    /// Whether the commit key can be pressed (the write's state aside).
    fn ready(&self, cx: &App) -> bool;
    fn commit_label(&self) -> SharedString;
    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>);
    /// The dialog's width; a table needs more than the fields' column.
    fn width(&self) -> f32 {
        DIALOG_WIDTH
    }
}

/// One field: its label above the control.
pub(crate) fn field(
    label: impl Into<SharedString>,
    control: impl IntoElement,
    colors: ShellColors,
) -> gpui::Div {
    v_flex()
        .w_full()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_medium()
                .text_color(colors.foreground)
                .child(label.into()),
        )
        .child(control)
}

/// A muted sentence inside a dialog.
pub(crate) fn note(text: impl Into<SharedString>, colors: ShellColors) -> gpui::Div {
    div().text_sm().text_color(colors.muted).child(text.into())
}

/// Opens `form` in the §10 frame.
pub(crate) fn show<F: DialogForm + 'static>(
    form: Entity<F>,
    title: &'static str,
    window: &mut Window,
    cx: &mut App,
) {
    window.open_dialog(cx, move |dialog, _, cx| {
        let colors = ShellColors::current(cx);
        let state = form.read(cx);
        let (busy, error) = {
            let status = state.status().read(cx);
            (status.busy, status.error.clone())
        };
        let ready = state.ready(cx) && !busy;
        let label = state.commit_label();
        let width = state.width();
        let weak = form.downgrade();
        let failure: Option<AnyElement> = error.map(|(sentence, detail)| {
            v_flex()
                .gap_1()
                .child(
                    div()
                        .debug_selector(|| "studio-dialog-error".into())
                        .text_sm()
                        .text_color(colors.danger)
                        .child(sentence),
                )
                .children(detail.map(|detail| {
                    h_flex().child(kit::technical("studio-dialog-technical", detail))
                }))
                .into_any_element()
        });
        let cancel = theme::clickable(
            Key::new("studio-dialog-cancel")
                .label(tr!("common-cancel"))
                .white()
                .disabled(busy)
                .debug_selector(|| "studio-dialog-cancel".into())
                .on_click(|_, window, cx| window.close_dialog(cx)),
            !busy,
        );
        let commit = theme::clickable(
            Key::new("studio-dialog-commit")
                .label(label)
                .primary()
                .disabled(!ready)
                .loading(busy)
                .debug_selector(|| "studio-dialog-commit".into())
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |form, cx| {
                        if form.ready(cx) && !form.status().read(cx).busy {
                            form.commit(window, cx);
                        }
                    });
                }),
            ready,
        );
        theme::dialog(dialog, cx)
            .w(px(width))
            .keyboard(!busy)
            .overlay_closable(!busy)
            .close_button(!busy)
            .title(title)
            .child(
                v_flex()
                    .w_full()
                    .gap_3()
                    .child(form.clone())
                    .children(failure),
            )
            .footer(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(div().flex_1())
                    .child(cancel)
                    .child(commit),
            )
    });
}
