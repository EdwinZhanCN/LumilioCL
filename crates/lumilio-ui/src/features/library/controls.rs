use gpui::prelude::*;
use gpui::{App, Context, Entity, Window};
use gpui_component::IndexPath;
use gpui_component::input::{InputEvent, InputState};
use gpui_component::select::{SearchableVec, SelectEvent, SelectState};

use super::library_actions::Library;
use super::order::{all_loaders, loader_code, present_loaders, sort_labels};
use super::page::{LIBRARY_LOADER, LIBRARY_SORT, PRESETS_TAB};
use crate::kit::ViewIntent;
use crate::live::loader_label;

type Labels = SearchableVec<String>;

/// The inputs the Library owns. Made the first time the page draws, because
/// they need a window.
pub struct LibraryControls {
    pub filter: Entity<InputState>,
    /// The Library's ordering and loader filter.
    pub sort: Entity<SelectState<Labels>>,
    pub loader: Entity<SelectState<Labels>>,
    /// The loader codes behind the loader list's entries after "全部".
    pub loaders: Vec<usize>,
    /// The language switch the placeholders and labels were taken in
    /// (`i18n::generation`).
    pub language: u32,
    /// The search box searches a preset's options (the 预设 tab) rather
    /// than games.
    presets_search: bool,
}

fn placeholder(presets: bool) -> &'static str {
    if presets {
        crate::tr!("preset-search")
    } else {
        crate::tr!("library-search")
    }
}

fn sorts() -> Vec<String> {
    sort_labels().iter().map(|s| (*s).to_owned()).collect()
}

impl LibraryControls {
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        Self {
            filter: cx.new(|cx| InputState::new(window, cx).placeholder(placeholder(false))),
            sort: cx.new(|cx| {
                SelectState::new(
                    SearchableVec::new(sorts()),
                    Some(IndexPath::default()),
                    window,
                    cx,
                )
            }),
            loader: cx.new(|cx| {
                SelectState::new(
                    SearchableVec::new(vec![all_loaders().to_owned()]),
                    Some(IndexPath::default()),
                    window,
                    cx,
                )
            }),
            loaders: Vec::new(),
            language: crate::i18n::generation(),
            presets_search: false,
        }
    }

    /// Names what the search box searches on the open tab.
    fn search_for(&mut self, presets: bool, window: &mut Window, cx: &mut App) {
        if self.presets_search != presets {
            self.presets_search = presets;
            self.filter.update(cx, |input, cx| {
                input.set_placeholder(placeholder(presets), window, cx)
            });
        }
    }

    /// Takes the placeholder and the dropdown labels again after a language
    /// switch, keeping what is typed and what is chosen. The loader list is
    /// marked stale, so it is rebuilt from the library.
    pub fn relabel(&mut self, window: &mut Window, cx: &mut App) {
        self.language = crate::i18n::generation();
        let placeholder = placeholder(self.presets_search);
        self.filter.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx)
        });
        let relabel = |select: &Entity<SelectState<Labels>>,
                       labels: Vec<String>,
                       window: &mut Window,
                       cx: &mut App| {
            select.update(cx, |select, cx| {
                let chosen = select.selected_index(cx);
                select.set_items(SearchableVec::new(labels), window, cx);
                select.set_selected_index(chosen, window, cx);
            });
        };
        relabel(&self.sort, sorts(), window, cx);
        relabel(&self.loader, vec![all_loaders().to_owned()], window, cx);
        self.loaders.clear();
    }
}

impl Library {
    /// Makes the controls the first time the page draws and listens to them;
    /// afterwards keeps them on the language, the tab and what is chosen.
    pub(super) fn ensure_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(controls) = &mut self.controls else {
            let controls = LibraryControls::new(window, cx);
            self.listen(&controls, window, cx);
            self.controls = Some(controls);
            return self.ensure_controls(window, cx);
        };
        if controls.language != crate::i18n::generation() {
            controls.relabel(window, cx);
        }
        controls.search_for(self.view.tab == PRESETS_TAB, window, cx);
        self.sync_dropdowns(window, cx);
    }

    fn listen(&self, controls: &LibraryControls, window: &mut Window, cx: &mut Context<Self>) {
        cx.subscribe(&controls.filter, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        cx.subscribe_in(
            &controls.sort,
            window,
            |this, _, event: &SelectEvent<Labels>, _, cx| {
                if let SelectEvent::Confirm(Some(label)) = event
                    && let Some(at) = sort_labels().iter().position(|sort| *sort == label)
                {
                    this.apply_view_intent(ViewIntent::Choose(LIBRARY_SORT, at), cx);
                }
            },
        )
        .detach();
        cx.subscribe_in(
            &controls.loader,
            window,
            |this, _, event: &SelectEvent<Labels>, _, cx| {
                let SelectEvent::Confirm(choice) = event else {
                    return;
                };
                let code = choice.as_ref().map_or(0, |label| {
                    present_loaders(&this.inputs.library)
                        .into_iter()
                        .find(|loader| loader_label(*loader) == label)
                        .map_or(0, loader_code)
                });
                this.apply_view_intent(ViewIntent::Choose(LIBRARY_LOADER, code), cx);
            },
        )
        .detach();
    }

    /// Keeps the two dropdowns on what is chosen and on the loaders the
    /// library holds.
    fn sync_dropdowns(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(controls) = &mut self.controls else {
            return;
        };
        let present = present_loaders(&self.inputs.library);
        let codes: Vec<usize> = present.iter().copied().map(loader_code).collect();
        let want_sort = self.view.sort.min(sort_labels().len() - 1);
        let want_loader = codes
            .iter()
            .position(|code| *code == self.view.loader)
            .map_or(0, |at| at + 1);
        let row = |select: &Entity<SelectState<Labels>>, cx: &App| {
            select.read(cx).selected_index(cx).map(|ix| ix.row)
        };
        let stale_list = controls.loaders != codes;
        if stale_list {
            let mut items = vec![all_loaders().to_owned()];
            items.extend(
                present
                    .iter()
                    .map(|loader| loader_label(*loader).to_owned()),
            );
            controls.loaders = codes;
            controls.loader.update(cx, |select, cx| {
                select.set_items(Labels::new(items), window, cx)
            });
        }
        if stale_list || row(&controls.loader, cx) != Some(want_loader) {
            controls.loader.update(cx, |select, cx| {
                select.set_selected_index(Some(IndexPath::new(want_loader)), window, cx)
            });
        }
        if row(&controls.sort, cx) != Some(want_sort) {
            controls.sort.update(cx, |select, cx| {
                select.set_selected_index(Some(IndexPath::new(want_sort)), window, cx)
            });
        }
    }
}
