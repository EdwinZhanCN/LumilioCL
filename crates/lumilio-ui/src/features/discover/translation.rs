//! Translation on Discover (plan discover-translation): MCIM's Chinese
//! summaries for the list and an open project's header, and the body when
//! the person asks. Every answer is checked against what is shown when it
//! arrives, and dropped if the page has moved on.

use std::time::Duration;

use gpui::{Context, Entity};
use lumilio_core::TextFormat;

use super::detail::ProjectDetailView;
use super::page::Discover;
use crate::runtime::answer;

/// How often a body translation looks whether it is still wanted.
const POLL: Duration = Duration::from_millis(600);

impl Discover {
    /// Asks MCIM for the summaries of one page of results from `source`, and
    /// shows those it has while the list is still search `seq`.
    pub(super) fn request_summaries(
        &mut self,
        seq: u64,
        source: String,
        items: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) {
        if items.is_empty() {
            return;
        }
        let Some(worker) = self.worker.clone() else {
            return;
        };
        let service = worker.service().clone();
        let asked = source.clone();
        let handle = worker.spawn(async move { service.translate_summaries(&asked, &items).await });
        cx.spawn(async move |page, cx| {
            let Some(found) = answer(handle, cx).await else {
                return;
            };
            let _ = page.update(cx, |page, cx| {
                if found.is_empty() || page.search_seq != seq {
                    return;
                }
                for row in &mut page.state.results {
                    if row.source == source
                        && let Some(text) = found.get(&row.project_id)
                    {
                        row.translated_summary = Some(text.clone());
                    }
                }
                page.state_changed(cx);
            });
        })
        .detach();
    }

    /// The summaries of the results already shown; for when translation is
    /// turned on after the search.
    pub(super) fn request_shown_summaries(&mut self, cx: &mut Context<Self>) {
        let source = self.state.query.source.clone();
        let items = self
            .state
            .results
            .iter()
            .filter(|row| row.source == source)
            .map(|row| (row.project_id.clone(), row.summary.clone()))
            .collect();
        let seq = self.search_seq;
        self.request_summaries(seq, source, items, cx);
    }

    /// Asks MCIM for each open project's summary, for its header.
    pub(super) fn request_detail_summaries(&mut self, cx: &mut Context<Self>) {
        let views: Vec<_> = self
            .details
            .iter()
            .filter_map(|(view, _)| view.upgrade())
            .collect();
        for view in views {
            self.request_detail_summary(&view, cx);
        }
    }

    /// Asks MCIM for the summary `view` shows, for its header.
    pub(super) fn request_detail_summary(
        &mut self,
        view: &Entity<ProjectDetailView>,
        cx: &mut Context<Self>,
    ) {
        let Some(query) = view.read(cx).summary_query() else {
            return;
        };
        let Some(worker) = self.worker.clone() else {
            return;
        };
        let service = worker.service().clone();
        let asked = query.clone();
        let handle = worker.spawn(async move {
            service
                .translate_summaries(&asked.source, &[(asked.project.clone(), asked.original)])
                .await
                .remove(&asked.project)
        });
        let detail = view.downgrade();
        cx.spawn(async move |_, cx| {
            let Some(translated) = answer(handle, cx).await else {
                return;
            };
            if let Some(view) = detail.upgrade() {
                view.update(cx, |view, cx| {
                    view.set_summary_translation(&query, translated, cx)
                });
            }
        })
        .detach();
    }

    /// Translates the open project's body. Leaving the page, or choosing the
    /// original meanwhile, stops the request; the page takes the answer only
    /// for the `request` it still waits for.
    pub(super) fn translate_body(
        &mut self,
        source: String,
        project: String,
        original: String,
        format: TextFormat,
        request: u64,
        cx: &mut Context<Self>,
    ) {
        // The detail that asked: it waits for this request, and is the one
        // on screen when the shell says which that is.
        let asking: Vec<_> = self
            .details
            .iter()
            .filter_map(|(view, _)| view.upgrade())
            .filter(|view| view.read(cx).translation_request() == Some(request))
            .collect();
        let Some(view) = asking
            .iter()
            .find(|view| Some(view.entity_id()) == self.shown_detail)
            .or(asking.first())
            .cloned()
        else {
            return;
        };
        let Some(worker) = self.worker.clone() else {
            return;
        };
        let service = worker.service().clone();
        let handle = worker.spawn(async move {
            service
                .translate_body(&source, &project, &original, format)
                .await
        });
        let id = view.entity_id();
        cx.spawn(async move |page, cx| {
            while !handle.is_finished() {
                cx.background_executor().timer(POLL).await;
                let wanted = page
                    .read_with(cx, |page, cx| {
                        page.shown_detail == Some(id)
                            && view.read(cx).translation_request() == Some(request)
                    })
                    .unwrap_or(false);
                if !wanted {
                    handle.abort();
                    return;
                }
            }
            if let Some(result) = answer(handle, cx).await {
                view.update(cx, |view, cx| view.set_translation(request, result, cx));
            }
        })
        .detach();
    }
}
