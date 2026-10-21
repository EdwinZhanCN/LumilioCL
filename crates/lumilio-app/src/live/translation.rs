//! Translation on Discover (plan discover-translation): MCIM's Chinese
//! summaries for the list and the open project's header, and the body when
//! the person asks. Every answer is checked against what is shown when it
//! arrives, and dropped if the page has moved on.

use super::jobs::{answer, job, outcome};
use super::{POLL, Reload, Wiring};
use gpui_kit::App;
use lumilio_core::TextFormat;
use lumilio_ui::tr;

/// Asks MCIM for the summaries of one page of results from `source`, and
/// shows those it has while the list is still search `seq`.
pub(super) fn request_summaries(
    wiring: &Wiring,
    seq: u64,
    source: String,
    items: Vec<(String, String)>,
    cx: &mut App,
) {
    if items.is_empty() {
        return;
    }
    let service = wiring.backend.service.clone();
    let asked = source.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.translate_summaries(&asked, &items).await });
    let wiring = wiring.clone();
    let (shell, state) = (wiring.shell.clone(), wiring.state.clone());
    cx.spawn(async move |cx| {
        let Some(found) = answer(&wiring, handle, cx).await else {
            return;
        };
        if found.is_empty() || state.borrow().search_seq != seq {
            return;
        }
        let _ = shell.update(cx, |shell, cx| {
            shell.update_live(
                |model| {
                    for row in &mut model.results {
                        if row.source == source
                            && let Some(text) = found.get(&row.project_id)
                        {
                            row.translated_summary = Some(text.clone());
                        }
                    }
                },
                cx,
            );
        });
    })
    .detach();
}

/// The summaries of the results already shown; for when translation is
/// turned on after the search.
pub(super) fn request_shown_summaries(wiring: &Wiring, cx: &mut App) {
    let shown = wiring
        .shell
        .read_with(cx, |shell, _| {
            let model = shell.live()?;
            let items = model
                .results
                .iter()
                .filter(|row| row.source == model.query.source)
                .map(|row| (row.project_id.clone(), row.summary.clone()))
                .collect::<Vec<_>>();
            Some((model.query.source.clone(), items))
        })
        .ok()
        .flatten();
    if let Some((source, items)) = shown {
        let seq = wiring.state.borrow().search_seq;
        request_summaries(wiring, seq, source, items, cx);
    }
}

/// Asks MCIM for the open project's summary, for its header.
pub(super) fn request_detail_summary(wiring: &Wiring, cx: &mut App) {
    let pending = wiring
        .shell
        .read_with(cx, |shell, cx| {
            let view = shell.detail_view()?;
            let query = view.read(cx).summary_query()?;
            Some((view, query))
        })
        .ok()
        .flatten();
    let Some((view, query)) = pending else {
        return;
    };
    let service = wiring.backend.service.clone();
    let asked = query.clone();
    let handle = wiring.backend.spawn(async move {
        service
            .translate_summaries(&asked.source, &[(asked.project.clone(), asked.original)])
            .await
            .remove(&asked.project)
    });
    let wiring = wiring.clone();
    let shell = wiring.shell.clone();
    cx.spawn(async move |cx| {
        let Some(translated) = answer(&wiring, handle, cx).await else {
            return;
        };
        let current = shell
            .read_with(cx, |shell, _| shell.detail_view())
            .ok()
            .flatten();
        if current.as_ref() != Some(&view) {
            return;
        }
        view.update(cx, |view, cx| {
            view.set_summary_translation(&query, translated, cx)
        });
    })
    .detach();
}

/// Translates the open project's body. Leaving the page, or choosing the
/// original meanwhile, stops the request; the page takes the answer only for
/// the `request` it still waits for.
pub(super) fn translate_body(
    wiring: &Wiring,
    source: String,
    project: String,
    original: String,
    format: TextFormat,
    request: u64,
    cx: &mut App,
) {
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        // Asked from inside the page's own update; read the shell a moment
        // later, as `search` does.
        let shell = wiring.shell.clone();
        let view = cx.update(|cx| {
            shell
                .read_with(cx, |shell, _| shell.detail_view())
                .ok()
                .flatten()
        });
        let Some(view) = view else { return };
        let service = wiring.backend.service.clone();
        let handle = wiring.backend.spawn(async move {
            service
                .translate_body(&source, &project, &original, format)
                .await
        });
        while !handle.is_finished() {
            cx.background_executor().timer(POLL).await;
            let wanted = cx.update(|cx| {
                shell
                    .read_with(cx, |shell, _| shell.detail_view())
                    .ok()
                    .flatten()
                    .as_ref()
                    == Some(&view)
                    && view.read(cx).translation_request() == Some(request)
            });
            if !wanted {
                handle.abort();
                return;
            }
        }
        if let Some(result) = answer(&wiring, handle, cx).await {
            view.update(cx, |view, cx| view.set_translation(request, result, cx));
        }
    })
    .detach();
}

pub(super) fn clear_cache(wiring: &Wiring, cx: &mut App) {
    let service = wiring.backend.service.clone();
    job(
        wiring,
        cx,
        false,
        Reload::Nothing,
        None,
        // Wrapped so that work which panicked fails like a refused clear.
        async move { Ok::<_, lumilio_core::ServiceError>(service.clear_translation_cache().await) },
        |shell, result, cx| {
            let result = result
                .map_err(|error| error.to_string())
                .and_then(|cleared| cleared.map_err(|error| error.to_string()));
            shell.toast(
                outcome(
                    result,
                    |()| tr!("translation-cache-cleared").to_owned(),
                    tr!("translation-cache-clear-failed").to_owned(),
                ),
                cx,
            );
        },
    );
}
