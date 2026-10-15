//! The studio in a window of its own (plan schematic-studio, T20; D22): a
//! title bar over the three columns. The application opens one window per
//! game and folder, hands it the file the pane showed, and gives it a
//! connection that belongs to this window alone.

use gpui::prelude::*;
use gpui::{Context, Entity, IntoElement, Render, Window, div, px};
use gpui_component::{ActiveTheme as _, TitleBar, v_flex};
use std::rc::Rc;

use super::{Act, Connection, LoadModel, Placement, StudioView};
use crate::tr;

/// The window's inset around the columns.
pub(super) const PADDING: f32 = 12.;

pub struct StudioWindow {
    studio: Entity<StudioView>,
}

impl StudioWindow {
    /// A studio on `plugin`'s `folder`, opened on `file` when there is one.
    /// `connection` is answered by the application; `load_model` and `act`
    /// reach it for this window's viewport and the plugin's own actions.
    pub fn new(
        plugin: String,
        folder: String,
        file: Option<String>,
        load_model: LoadModel,
        act: Act,
        connection: Connection,
        cx: &mut Context<Self>,
    ) -> Self {
        let studio = cx.new(|cx| {
            StudioView::new(
                plugin,
                folder,
                Placement::Window,
                Rc::new(|_, _, _| {}),
                load_model,
                act,
                cx,
            )
        });
        studio.update(cx, |studio, cx| {
            studio.connect(connection, cx);
            if let Some(file) = file {
                studio.select_file(file, cx);
            }
        });
        Self { studio }
    }

    pub fn studio(&self) -> &Entity<StudioView> {
        &self.studio
    }
}

impl Render for StudioWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(TitleBar::new().child(div().text_sm().child(tr!("studio-window-title"))))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .p(px(PADDING))
                    .child(self.studio.clone()),
            )
    }
}
