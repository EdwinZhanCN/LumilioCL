use gpui::prelude::*;
use gpui::{App, Entity, Window};
use gpui_component::IndexPath;
use gpui_component::select::{SearchableVec, SelectState};

use super::super::ViewState;
use crate::kit::Emit;
use crate::live::{LiveHandler, LiveIntent, LiveModel};
use crate::theme::ShellColors;

/// Inputs the live pages own. Created lazily, because they need a window.
pub struct LiveControls {
    /// The Settings translation provider; Microsoft Edge is the only one.
    pub translation_provider: Entity<SelectState<SearchableVec<String>>>,
    /// The Settings plugin list's scroll, so it keeps the wheel while it can
    /// scroll.
    pub plugin_scroll: gpui::ScrollHandle,
}

impl LiveControls {
    pub fn new(window: &mut Window, cx: &mut gpui::Context<Self>) -> Self {
        Self {
            translation_provider: cx.new(|cx| {
                SelectState::new(
                    SearchableVec::new(vec!["Microsoft Edge".to_owned()]),
                    Some(IndexPath::default()),
                    window,
                    cx,
                )
            }),
            plugin_scroll: gpui::ScrollHandle::new(),
        }
    }
}

pub struct LiveCtx<'a> {
    pub colors: ShellColors,
    pub model: &'a LiveModel,
    pub state: &'a ViewState,
    pub emit: Emit,
    pub handler: LiveHandler,
    pub controls: &'a LiveControls,
    /// The window's size, for layouts that change with the room they have.
    pub window_width: f32,
    pub window_height: f32,
    /// The look preview of the account the Accounts detail shows.
    pub account_viewer: Option<&'a gpui::Entity<crate::skin_view::SkinViewer>>,
    /// Its look area (the preview beside the skins), unless it is read-only.
    pub wardrobe: Option<&'a gpui::Entity<crate::wardrobe::Wardrobe>>,
    /// The look in force and the themes found, for the 外观 tab.
    pub look: Option<&'a crate::theme::Look>,
    pub catalog: Option<&'a crate::theme::Catalog>,
    pub wallpaper: Option<&'a crate::theme::Wallpaper>,
}

pub(crate) fn send(
    handler: &LiveHandler,
    intent: LiveIntent,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |window, cx| handler(intent.clone(), window, cx)
}
