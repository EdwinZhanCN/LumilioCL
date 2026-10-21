use super::super::ViewState;
use super::library::sort_labels;
use crate::kit::Emit;
use crate::live::{
    DiscoverChange, LiveHandler, LiveIntent, LiveModel, PAGE_SIZES, SORTS, sort_label,
};
use crate::theme::ShellColors;
use gpui::prelude::*;
use gpui::{App, Entity, Window};
use gpui_component::IndexPath;
use gpui_component::input::InputState;
use gpui_component::select::{SearchableVec, SelectState};

/// Inputs the live pages own. Created lazily, because they need a window.
pub struct LiveControls {
    pub library_filter: Entity<InputState>,
    pub discover_search: Entity<InputState>,
    pub sort: Entity<SelectState<SearchableVec<String>>>,
    pub page_size: Entity<SelectState<SearchableVec<String>>>,
    /// The box that narrows the sidebar's version list.
    pub version_search: Entity<InputState>,
    /// The version list's scroll, so it keeps the wheel while it can scroll.
    pub version_scroll: gpui::ScrollHandle,
    /// The scroll of the filters panel a narrow window opens from its key.
    pub filters_scroll: gpui::ScrollHandle,
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
    /// The source Discover's search box names.
    discover_source: String,
    /// The sorts the Discover sort menu offers (the source's).
    discover_sorts: Vec<lumilio_core::SortIndex>,
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
            discover_search: cx.new(|cx| {
                InputState::new(window, cx).placeholder(crate::tr!("discover-search", source = ""))
            }),
            sort: cx.new(|cx| {
                let labels: Vec<String> = SORTS.iter().map(|s| sort_label(*s).to_owned()).collect();
                SelectState::new(
                    SearchableVec::new(labels),
                    Some(IndexPath::default()),
                    window,
                    cx,
                )
            }),
            page_size: cx.new(|cx| {
                let labels: Vec<String> = PAGE_SIZES.iter().map(u32::to_string).collect();
                let at = PAGE_SIZES.iter().position(|size| *size == 20).unwrap_or(0);
                SelectState::new(
                    SearchableVec::new(labels),
                    Some(IndexPath::new(at)),
                    window,
                    cx,
                )
            }),
            version_search: cx.new(|cx| {
                InputState::new(window, cx).placeholder(crate::tr!("discover-version-search"))
            }),
            version_scroll: gpui::ScrollHandle::new(),
            filters_scroll: gpui::ScrollHandle::new(),
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
            discover_source: String::new(),
            discover_sorts: SORTS.to_vec(),
        }
    }

    /// Names the source Discover's search box searches.
    pub fn search_source(&mut self, name: &str, window: &mut Window, cx: &mut gpui::Context<Self>) {
        if self.discover_source != name {
            self.discover_source = name.to_owned();
            let placeholder = crate::tr!("discover-search", source = name);
            self.discover_search.update(cx, |input, cx| {
                input.set_placeholder(placeholder, window, cx)
            });
        }
    }

    /// Offers the sorts the source has, with `chosen` selected.
    pub fn offer_sorts(
        &mut self,
        sorts: Vec<lumilio_core::SortIndex>,
        chosen: lumilio_core::SortIndex,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let at = sorts.iter().position(|sort| *sort == chosen).unwrap_or(0);
        let changed = self.discover_sorts != sorts;
        let selected = self.sort.read(cx).selected_index(cx).map(|ix| ix.row);
        if !changed && selected == Some(at) {
            return;
        }
        if changed {
            let labels: Vec<String> = sorts.iter().map(|s| sort_label(*s).to_owned()).collect();
            self.sort.update(cx, |select, cx| {
                select.set_items(SearchableVec::new(labels), window, cx)
            });
            self.discover_sorts = sorts;
        }
        self.sort.update(cx, |select, cx| {
            select.set_selected_index(Some(IndexPath::new(at)), window, cx)
        });
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
        for (input, placeholder) in [
            (
                &self.library_filter,
                library_placeholder(self.presets_search).to_owned(),
            ),
            (
                &self.discover_search,
                crate::tr!("discover-search", source = self.discover_source.as_str()),
            ),
            (
                &self.version_search,
                crate::tr!("discover-version-search").to_owned(),
            ),
        ] {
            input.update(cx, |input, cx| {
                input.set_placeholder(placeholder, window, cx)
            });
        }
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
        let sorts = self
            .discover_sorts
            .iter()
            .map(|s| sort_label(*s).to_owned())
            .collect();
        relabel(&self.sort, sorts, window, cx);
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

pub type DiscoverChangeHandler = std::rc::Rc<dyn Fn(DiscoverChange, &mut Window, &mut App)>;

pub struct LiveCtx<'a> {
    pub colors: ShellColors,
    pub model: &'a LiveModel,
    pub state: &'a ViewState,
    pub emit: Emit,
    pub handler: LiveHandler,
    /// Applies a change to the Discover query and searches again.
    pub change: DiscoverChangeHandler,
    pub controls: &'a LiveControls,
    pub filter: String,
    /// What is typed in the sidebar's version search.
    pub version_filter: String,
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

pub(super) fn send(
    handler: &LiveHandler,
    intent: LiveIntent,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |window, cx| handler(intent.clone(), window, cx)
}
