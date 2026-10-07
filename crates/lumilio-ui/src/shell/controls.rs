use super::{Labels, LauncherShell};
use crate::kit::ViewIntent;
use crate::live::{DiscoverChange, PAGE_SIZES, SORTS, sort_label};
use crate::pages;
use crate::pages::live::LiveControls;
use gpui::prelude::*;
use gpui::{App, Context, Entity, Window};
use gpui_component::input::InputEvent;
use gpui_component::select::SelectEvent;

impl LauncherShell {
    /// Builds the Library and Discover controls the first time the live pages show,
    /// and listens to them.
    pub(super) fn ensure_live_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.live.is_some() && self.live_controls.is_none() {
            let controls = cx.new(|cx| LiveControls::new(window, cx));
            let (search, filter) = {
                let read = controls.read(cx);
                (read.discover_search.clone(), read.library_filter.clone())
            };
            cx.subscribe_in(
                &search,
                window,
                |this, _, event: &InputEvent, window, cx| {
                    match event {
                        InputEvent::PressEnter { .. } => this.submit_search(window, cx),
                        // The clear button empties the box: search again, as the app does.
                        InputEvent::Change if this.discover_text_was_cleared(cx) => {
                            this.submit_search(window, cx)
                        }
                        _ => {}
                    }
                },
            )
            .detach();
            cx.subscribe(&filter, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            })
            .detach();
            let (sort, size, version_search) = {
                let read = controls.read(cx);
                (
                    read.sort.clone(),
                    read.page_size.clone(),
                    read.version_search.clone(),
                )
            };
            cx.subscribe_in(
                &sort,
                window,
                |this, _, event: &SelectEvent<Labels>, window, cx| {
                    if let SelectEvent::Confirm(Some(label)) = event
                        && let Some(sort) = SORTS.iter().find(|sort| sort_label(**sort) == label)
                    {
                        this.change_query(DiscoverChange::Sort(*sort), window, cx);
                    }
                },
            )
            .detach();
            cx.subscribe_in(
                &size,
                window,
                |this, _, event: &SelectEvent<Labels>, window, cx| {
                    if let SelectEvent::Confirm(Some(text)) = event
                        && let Some(size) = PAGE_SIZES.iter().find(|size| size.to_string() == *text)
                    {
                        this.change_query(DiscoverChange::PageSize(*size), window, cx);
                    }
                },
            )
            .detach();
            cx.subscribe(&version_search, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            })
            .detach();
            let (library_sort, library_loader) = {
                let read = controls.read(cx);
                (read.library_sort.clone(), read.library_loader.clone())
            };
            cx.subscribe_in(
                &library_sort,
                window,
                |this, _, event: &SelectEvent<Labels>, window, cx| {
                    if let SelectEvent::Confirm(Some(label)) = event
                        && let Some(at) = pages::live::SORT_LABELS
                            .iter()
                            .position(|sort| *sort == label)
                    {
                        let intent = ViewIntent::Choose(pages::live::LIBRARY_SORT, at);
                        this.apply_view_intent(intent, cx);
                        this.remember_library_view(intent, window, cx);
                    }
                },
            )
            .detach();
            cx.subscribe_in(
                &library_loader,
                window,
                |this, _, event: &SelectEvent<Labels>, window, cx| {
                    let SelectEvent::Confirm(choice) = event else {
                        return;
                    };
                    let code = choice.as_ref().map_or(0, |label| {
                        this.live
                            .as_ref()
                            .map(|model| pages::live::present_loaders(&model.library))
                            .and_then(|loaders| {
                                loaders
                                    .into_iter()
                                    .find(|loader| crate::live::loader_label(*loader) == label)
                            })
                            .map_or(0, pages::live::loader_code)
                    });
                    let intent = ViewIntent::Choose(pages::live::LIBRARY_LOADER, code);
                    this.apply_view_intent(intent, cx);
                    this.remember_library_view(intent, window, cx);
                },
            )
            .detach();
            self.live_controls = Some(controls);
        }
    }

    /// Keeps the Library's two dropdowns on what is remembered and on the loaders
    /// the library holds.
    pub(super) fn sync_library_dropdowns(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The Library's two dropdowns follow what is remembered and what the
        // library holds.
        if let (Some(controls), Some(model)) = (&self.live_controls, &self.live) {
            use gpui_component::IndexPath;
            use pages::live::{ALL_LOADERS, LIBRARY_LOADER, LIBRARY_SORT, SORT_LABELS};
            let present = pages::live::present_loaders(&model.library);
            let codes: Vec<usize> = present
                .iter()
                .copied()
                .map(pages::live::loader_code)
                .collect();
            let want_sort = self.view.choice(LIBRARY_SORT, 0).min(SORT_LABELS.len() - 1);
            let code = self.view.choice(LIBRARY_LOADER, 0);
            let want_loader = codes.iter().position(|c| *c == code).map_or(0, |at| at + 1);
            let read = controls.read(cx);
            let row = |select: &Entity<gpui_component::select::SelectState<Labels>>, cx: &App| {
                select.read(cx).selected_index(cx).map(|ix| ix.row)
            };
            let stale_list = read.library_loaders != codes;
            let stale_sort = row(&read.library_sort, cx) != Some(want_sort);
            let stale_loader = stale_list || row(&read.library_loader, cx) != Some(want_loader);
            if stale_list || stale_sort || stale_loader {
                controls.update(cx, |controls, cx| {
                    if stale_list {
                        let mut items = vec![ALL_LOADERS.to_owned()];
                        items.extend(
                            present
                                .iter()
                                .map(|loader| crate::live::loader_label(*loader).to_owned()),
                        );
                        controls.library_loaders = codes;
                        controls.library_loader.update(cx, |select, cx| {
                            select.set_items(Labels::new(items), window, cx)
                        });
                    }
                    if stale_loader {
                        controls.library_loader.update(cx, |select, cx| {
                            select.set_selected_index(Some(IndexPath::new(want_loader)), window, cx)
                        });
                    }
                    if stale_sort {
                        controls.library_sort.update(cx, |select, cx| {
                            select.set_selected_index(Some(IndexPath::new(want_sort)), window, cx)
                        });
                    }
                });
            }
        }
    }
}
