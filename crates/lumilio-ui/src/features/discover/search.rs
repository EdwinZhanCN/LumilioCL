use std::future::Future;

use gpui::Context;
use lumilio_core::{ProjectKind, ServiceError, unix_now};
use tokio::task::JoinError;

use super::model::{FilterModel, SearchStatus, search_rows};
use super::page::{Discover, DiscoverEvent};
use crate::runtime::{self, answer};
use crate::toast::Toast;

impl Discover {
    /// Runs `work` on the launcher's runtime, then hands the result to `done`
    /// on the interface thread. `started` floats up when it begins; work
    /// that shows in Activity (`tasks`) tells it to look again meanwhile and
    /// when it ends. Work that panics fails like any other, so `done` still
    /// runs and says so. The page outlives the work: the shell keeps it for
    /// as long as the window is open.
    pub(super) fn job<T, E>(
        &mut self,
        cx: &mut Context<Self>,
        tasks: bool,
        started: Option<Toast>,
        work: impl Future<Output = Result<T, E>> + Send + 'static,
        done: impl FnOnce(&mut Self, Result<T, E>, &mut Context<Self>) + 'static,
    ) where
        T: Send + 'static,
        E: From<JoinError> + Send + 'static,
    {
        let Some(worker) = &self.worker else {
            return;
        };
        let tasks = tasks.then_some(DiscoverEvent::TasksChanged);
        runtime::job(worker, cx, tasks, started, work, move |page, output, cx| {
            done(page, output, cx);
            page.refresh_installed(cx);
        });
    }

    /// Runs `query`. A slow answer cannot replace a newer search's.
    pub(super) fn search(&mut self, query: &super::model::DiscoverQuery, cx: &mut Context<Self>) {
        self.search_seq += 1;
        let seq = self.search_seq;
        let kind = query.kind;
        let provided = self.state.provided();
        let filters = self.state.filters.clone();
        let hide = self.state.hiding_installed();
        let game = self.state.browsing_for.clone();
        let mut known = self.state.installed_projects();
        let known_is_current = kind == ProjectKind::Modpack
            || game.as_ref().is_some_and(|game| {
                self.installed_key.as_ref() == Some(&(game.clone(), kind, query.source.clone()))
            });
        let Some(service) = self.worker.as_ref().map(|worker| worker.service().clone()) else {
            return;
        };
        let asked = query.clone();
        let source = query.source.clone();
        self.job(
            cx,
            false,
            None,
            async move {
                // Leaving out what is already installed needs the list at the
                // time of searching; a failure to read it just leaves nothing out.
                let mut hidden = Vec::new();
                if hide {
                    hidden = std::mem::take(&mut known);
                    if !known_is_current && let Some(game) = &game {
                        hidden = service
                            .installed_projects(game, kind, &asked.source)
                            .await
                            .map(|found| found.into_keys().collect())
                            .unwrap_or_default();
                    }
                }
                let search = asked.to_search(&provided, &filters, hidden);
                service.search(&asked.source, &search).await
            },
            move |page, result, cx| {
                if page.search_seq != seq {
                    return;
                }
                let now = unix_now();
                if let Ok(found) = &result {
                    let items = found
                        .hits
                        .iter()
                        .map(|hit| (hit.project_id.clone(), hit.description.clone()))
                        .collect();
                    page.request_summaries(seq, source, items, cx);
                }
                let state = &mut page.state;
                match result {
                    Ok(found) => {
                        state.results = search_rows(&found, now, &state.query, &state.filters);
                        state.search = SearchStatus::Done {
                            total: found.total_hits,
                        };
                    }
                    Err(ServiceError::NoContentSource) => {
                        state.results.clear();
                        state.search = SearchStatus::NoSource;
                    }
                    Err(error) => {
                        state.results.clear();
                        state.search = SearchStatus::Failed(error.to_string());
                    }
                }
                page.state_changed(cx);
            },
        );
    }

    /// Loads the content sources that are on and the filter choices of the
    /// one browsed. A remembered source that is no longer on gives way to the
    /// first one that is, and the list is searched again there.
    pub(super) fn load_filters(&mut self, cx: &mut Context<Self>) {
        let Some(service) = self.worker.as_ref().map(|worker| worker.service().clone()) else {
            return;
        };
        let wanted = self.state.query.source.clone();
        let asked = wanted.clone();
        self.job(
            cx,
            false,
            None,
            async move {
                let sources = service.content_sources().await;
                let source = sources
                    .iter()
                    .find(|source| source.id == wanted)
                    .or_else(|| sources.first())
                    .map_or(wanted.clone(), |source| source.id.clone());
                let filters = service.discover_filters(&source).await;
                let moved = wanted != source;
                Ok::<_, ServiceError>((sources, source, moved, filters))
            },
            move |page, loaded, cx| {
                let state = &mut page.state;
                let (sources, source, moved, result) = match loaded {
                    Ok(loaded) => loaded,
                    Err(error) => {
                        state.filters.loaded = true;
                        state.filters.error = Some(error.to_string());
                        page.state_changed(cx);
                        return;
                    }
                };
                // Another source was chosen while these loaded; its own load
                // is on the way, and these choices would put the old one back.
                if state.query.source != asked {
                    state.sources = sources;
                    page.state_changed(cx);
                    return;
                }
                state.sources = sources;
                if state.query.source != source {
                    state.query.source = source.clone();
                }
                match result {
                    Ok(filters) => state.filters = FilterModel::from_core(&filters),
                    // The page says so where the results go; no failed-filters note.
                    Err(ServiceError::NoContentSource) => state.filters.loaded = true,
                    Err(error) => {
                        state.filters.loaded = true;
                        state.filters.error = Some(error.to_string());
                    }
                }
                page.state_changed(cx);
                if moved {
                    let query = page.state.query.clone();
                    page.search(&query, cx);
                }
            },
        );
    }

    /// Which content sources are on may have changed: once the page has
    /// listed them, it reads them and the browsed source's filters again.
    pub fn sources_changed(&mut self, cx: &mut Context<Self>) {
        if !self.state.sources.is_empty() {
            self.load_filters(cx);
        }
    }

    /// The first visit loads the filter choices and lists what is popular.
    pub(super) fn first_search(&mut self, cx: &mut Context<Self>) {
        if self.state.search != SearchStatus::Idle {
            return;
        }
        self.state.search = SearchStatus::Searching;
        self.state_changed(cx);
        let query = self.state.query.clone();
        self.load_filters(cx);
        self.search(&query, cx);
    }

    /// What the target game has must be read again (it changed outside the
    /// page).
    pub fn invalidate_installed(&mut self, cx: &mut Context<Self>) {
        self.installed_key = None;
        self.refresh_installed(cx);
    }

    /// Reads again what the target game already has of the searched kind,
    /// when the game or the kind changed or something was installed. Failing
    /// to ask just shows nothing as installed.
    pub fn refresh_installed(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.state.target.clone() else {
            return;
        };
        let (kind, source) = (self.state.query.kind, self.state.query.source.clone());
        if kind == ProjectKind::Modpack {
            self.installed_key = None;
            if !self.state.installed.is_empty() {
                self.state.installed.clear();
                self.state_changed(cx);
            }
            return;
        }
        let key = (target.clone(), kind, source.clone());
        if self.installed_key.as_ref() == Some(&key) {
            return;
        }
        self.installed_key = Some(key);
        let Some(worker) = self.worker.clone() else {
            return;
        };
        let service = worker.service().clone();
        let (asked, from) = (target.clone(), source.clone());
        let handle =
            worker.spawn(async move { service.installed_projects(&asked, kind, &from).await });
        cx.spawn(async move |page, cx| {
            let found = answer(handle, cx)
                .await
                .and_then(Result::ok)
                .unwrap_or_default();
            let _ = page.update(cx, |page, cx| {
                // Only if still about the same game, kind and source.
                let state = &mut page.state;
                if state.target.as_deref() == Some(target.as_str())
                    && state.query.kind == kind
                    && state.query.source == source
                {
                    state.installed = found;
                    page.state_changed(cx);
                }
            });
        })
        .detach();
    }
}
