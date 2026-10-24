use gpui::prelude::*;
use gpui::{Entity, ScrollHandle, Window};
use gpui_component::IndexPath;
use gpui_component::input::InputState;
use gpui_component::select::{SearchableVec, SelectState};
use lumilio_core::SortIndex;

use super::model::{PAGE_SIZES, SORTS, sort_label};

pub(super) type Labels = SearchableVec<String>;

/// The inputs Discover owns. Made the first time the page draws, because
/// they need a window.
pub struct DiscoverControls {
    pub search: Entity<InputState>,
    pub sort: Entity<SelectState<Labels>>,
    pub page_size: Entity<SelectState<Labels>>,
    /// The box that narrows the sidebar's version list.
    pub version_search: Entity<InputState>,
    /// The version list's scroll, so it keeps the wheel while it can scroll.
    pub version_scroll: ScrollHandle,
    /// The scroll of the filters panel a narrow window opens from its key.
    pub filters_scroll: ScrollHandle,
    /// The language switch the placeholders and labels were taken in
    /// (`i18n::generation`).
    pub(super) language: u32,
    /// The source the search box names.
    source: String,
    /// The sorts the sort menu offers (the source's).
    sorts: Vec<SortIndex>,
}

impl DiscoverControls {
    pub fn new(window: &mut Window, cx: &mut gpui::App) -> Self {
        Self {
            search: cx.new(|cx| {
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
            version_scroll: ScrollHandle::new(),
            filters_scroll: ScrollHandle::new(),
            language: crate::i18n::generation(),
            source: String::new(),
            sorts: SORTS.to_vec(),
        }
    }

    /// Names the source the search box searches.
    pub(super) fn search_source(&mut self, name: &str, window: &mut Window, cx: &mut gpui::App) {
        if self.source != name {
            self.source = name.to_owned();
            let placeholder = crate::tr!("discover-search", source = name);
            self.search.update(cx, |input, cx| {
                input.set_placeholder(placeholder, window, cx)
            });
        }
    }

    /// Offers the sorts the source has, with `chosen` selected.
    pub(super) fn offer_sorts(
        &mut self,
        sorts: Vec<SortIndex>,
        chosen: SortIndex,
        window: &mut Window,
        cx: &mut gpui::App,
    ) {
        let at = sorts.iter().position(|sort| *sort == chosen).unwrap_or(0);
        let changed = self.sorts != sorts;
        let selected = self.sort.read(cx).selected_index(cx).map(|ix| ix.row);
        if !changed && selected == Some(at) {
            return;
        }
        if changed {
            let labels: Vec<String> = sorts.iter().map(|s| sort_label(*s).to_owned()).collect();
            self.sort.update(cx, |select, cx| {
                select.set_items(SearchableVec::new(labels), window, cx)
            });
            self.sorts = sorts;
        }
        self.sort.update(cx, |select, cx| {
            select.set_selected_index(Some(IndexPath::new(at)), window, cx)
        });
    }

    /// Takes the placeholders and the sort labels again after a language
    /// switch, keeping what is typed and what is chosen.
    pub(super) fn relabel(&mut self, window: &mut Window, cx: &mut gpui::App) {
        self.language = crate::i18n::generation();
        for (input, placeholder) in [
            (
                &self.search,
                crate::tr!("discover-search", source = self.source.as_str()),
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
        let labels: Vec<String> = self
            .sorts
            .iter()
            .map(|s| sort_label(*s).to_owned())
            .collect();
        self.sort.update(cx, |select, cx| {
            let chosen = select.selected_index(cx);
            select.set_items(SearchableVec::new(labels), window, cx);
            select.set_selected_index(chosen, window, cx);
        });
    }

    /// The search box and the sort menu go back to their starting values (a
    /// new kind of content is a new search).
    pub(super) fn reset(&self, window: &mut Window, cx: &mut gpui::App) {
        self.search
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.sort.update(cx, |select, cx| {
            select.set_selected_index(Some(IndexPath::default()), window, cx)
        });
    }
}
