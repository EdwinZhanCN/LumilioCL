use gpui::prelude::*;
use gpui::{App, Entity, Window};
use gpui_component::IndexPath;
use gpui_component::input::InputState;
use gpui_component::select::{SearchableVec, SelectState};

use super::super::ViewState;
use super::library::sort_labels;
use crate::kit::Emit;
use crate::live::{LiveHandler, LiveIntent, LiveModel};
use crate::theme::ShellColors;

/// Inputs the live pages own. Created lazily, because they need a window.
pub struct LiveControls {
    pub library_filter: Entity<InputState>,
    /// The Library's ordering and loader filter.
    pub library_sort: Entity<SelectState<SearchableVec<String>>>,
    pub library_loader: Entity<SelectState<SearchableVec<String>>>,
    /// The loader codes behind the loader list's entries after "全部".
    pub library_loaders: Vec<usize>,
    /// The Settings translation provider; Microsoft Edge is the only one.
    pub translation_provider: Entity<SelectState<SearchableVec<String>>>,
    /// The Settings plugin list's scroll, so it keeps the wheel while it can
    /// scroll.
    pub plugin_scroll: gpui::ScrollHandle,
    /// The language switch the placeholders and labels were taken in
    /// (`i18n::generation`).
    pub language: u32,
    /// The Library search box searches a preset's options (the 预设 tab)
    /// rather than games.
    presets_search: bool,
}

fn library_placeholder(presets: bool) -> &'static str {
    if presets {
        crate::tr!("preset-search")
    } else {
        crate::tr!("library-search")
    }
}

/// The text of the "no loader filter" entry.
#[must_use]
pub fn all_loaders() -> &'static str {
    crate::tr!("library-all-loaders")
}

impl LiveControls {
    pub fn new(window: &mut Window, cx: &mut gpui::Context<Self>) -> Self {
        Self {
            library_filter: cx
                .new(|cx| InputState::new(window, cx).placeholder(crate::tr!("library-search"))),
            library_sort: cx.new(|cx| {
                let labels: Vec<String> = sort_labels().iter().map(|s| (*s).to_owned()).collect();
                SelectState::new(
                    SearchableVec::new(labels),
                    Some(IndexPath::default()),
                    window,
                    cx,
                )
            }),
            library_loader: cx.new(|cx| {
                SelectState::new(
                    SearchableVec::new(vec![all_loaders().to_owned()]),
                    Some(IndexPath::default()),
                    window,
                    cx,
                )
            }),
            library_loaders: Vec::new(),
            translation_provider: cx.new(|cx| {
                SelectState::new(
                    SearchableVec::new(vec!["Microsoft Edge".to_owned()]),
                    Some(IndexPath::default()),
                    window,
                    cx,
                )
            }),
            plugin_scroll: gpui::ScrollHandle::new(),
            language: crate::i18n::generation(),
            presets_search: false,
        }
    }

    /// Names what the Library search box searches on the open tab.
    pub fn search_for(&mut self, presets: bool, window: &mut Window, cx: &mut gpui::Context<Self>) {
        if self.presets_search != presets {
            self.presets_search = presets;
            self.library_filter.update(cx, |input, cx| {
                input.set_placeholder(library_placeholder(presets), window, cx)
            });
        }
    }

    /// Takes the placeholders and the dropdown labels again after a language
    /// switch, keeping what is typed and what is chosen. The loader list is
    /// marked stale, so the shell rebuilds it from the library.
    pub fn relabel(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
        self.language = crate::i18n::generation();
        let placeholder = library_placeholder(self.presets_search);
        self.library_filter.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx)
        });
        let relabel = |select: &Entity<SelectState<SearchableVec<String>>>,
                       labels: Vec<String>,
                       window: &mut Window,
                       cx: &mut gpui::Context<Self>| {
            select.update(cx, |select, cx| {
                let chosen = select.selected_index(cx);
                select.set_items(SearchableVec::new(labels), window, cx);
                select.set_selected_index(chosen, window, cx);
            });
        };
        let library_sorts = sort_labels().iter().map(|s| (*s).to_owned()).collect();
        relabel(&self.library_sort, library_sorts, window, cx);
        relabel(
            &self.library_loader,
            vec![all_loaders().to_owned()],
            window,
            cx,
        );
        self.library_loaders.clear();
    }
}

pub struct LiveCtx<'a> {
    pub colors: ShellColors,
    pub model: &'a LiveModel,
    pub state: &'a ViewState,
    pub emit: Emit,
    pub handler: LiveHandler,
    pub controls: &'a LiveControls,
    pub filter: String,
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
