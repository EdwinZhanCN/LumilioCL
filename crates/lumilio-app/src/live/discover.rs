use super::jobs::{job, outcome};
use super::{Reload, Wiring};
use gpui_kit::AppContext as _;
use gpui_kit::{App, WeakEntity, Window};
use lumilio_core::{CancellationToken, ProjectKind, ServiceError, unix_now};
use lumilio_ui::LauncherShell;
use lumilio_ui::dependency_prompt::DependencyPrompt;
use lumilio_ui::live::{DiscoverQuery, FilterModel, Provided, SearchStatus, search_rows};
use lumilio_ui::project_detail::DetailState;
use lumilio_ui::toast::Toast;
use lumilio_ui::tr;
use std::rc::Rc;

/// What a search needs from the page besides its query.
#[derive(Default)]
struct SearchPlan {
    provided: Provided,
    filters: FilterModel,
    /// "Hide already installed" is on.
    hide: bool,
    /// The game being browsed for, when there is one.
    game: Option<String>,
    /// What the page already knows is installed (packs in the library, or
    /// the game's content when it was read for this kind).
    known: Vec<String>,
    known_is_current: bool,
}

pub(super) fn search(wiring: &Wiring, query: &DiscoverQuery, cx: &mut App) {
    let seq = {
        let mut state = wiring.state.borrow_mut();
        state.search_seq += 1;
        state.search_seq
    };
    // The shell may be in the middle of its own update (it asks for searches
    // from there), and the plan reads the shell: do that a moment later.
    let (wiring, query) = (wiring.clone(), query.clone());
    cx.spawn(async move |cx| cx.update(|cx| run_search(&wiring, &query, seq, cx)))
        .detach();
}

fn run_search(wiring: &Wiring, query: &DiscoverQuery, seq: u64, cx: &mut App) {
    let kind = query.kind;
    let plan = wiring
        .shell
        .read_with(cx, |shell, _| {
            shell.live().map(|model| SearchPlan {
                provided: model.provided(),
                filters: model.filters.clone(),
                hide: model.hiding_installed(),
                game: model.browsing_for.clone(),
                known: model.installed_projects(),
                known_is_current: kind == ProjectKind::Modpack,
            })
        })
        .ok()
        .flatten()
        .unwrap_or_default();
    let known_is_current = plan.known_is_current
        || plan.game.as_ref().is_some_and(|game| {
            wiring.state.borrow().installed_key.as_ref() == Some(&(game.clone(), kind))
        });
    let service = wiring.backend.service.clone();
    let state = wiring.state.clone();
    let asked = query.clone();
    let translated_wiring = wiring.clone();
    job(
        wiring,
        cx,
        false,
        Reload::Nothing,
        None,
        async move {
            // Leaving out what is already installed needs the list at the time
            // of searching; a failure to read it just leaves nothing out.
            let mut hidden = Vec::new();
            if plan.hide {
                hidden = plan.known;
                if !known_is_current && let Some(game) = &plan.game {
                    hidden = service
                        .installed_projects(game, kind)
                        .await
                        .map(|found| found.into_keys().collect())
                        .unwrap_or_default();
                }
            }
            let search = asked.to_search(&plan.provided, &plan.filters, hidden);
            service.search(&search).await
        },
        move |shell, result, cx| {
            if state.borrow().search_seq != seq {
                return;
            }
            let now = unix_now();
            shell.update_live(
                |model| match result {
                    Ok(page) => {
                        model.results = search_rows(&page, now, &model.query, &model.filters);
                        model.search = SearchStatus::Done {
                            total: page.total_hits,
                        };
                    }
                    Err(ServiceError::NoContentSource) => {
                        model.results.clear();
                        model.search = SearchStatus::NoSource;
                    }
                    Err(error) => {
                        model.results.clear();
                        model.search = SearchStatus::Failed(error.to_string());
                    }
                },
                cx,
            );
            request_summaries(&translated_wiring, seq, cx);
        },
    );
}

pub(super) fn load_filters(wiring: &Wiring, cx: &mut App) {
    let service = wiring.backend.service.clone();
    job(
        wiring,
        cx,
        false,
        Reload::Nothing,
        None,
        async move { service.discover_filters().await },
        |shell, result, cx| {
            shell.update_live(
                |model| match result {
                    Ok(filters) => model.filters = FilterModel::from_core(&filters),
                    // The page says so where the results go; no failed-filters note.
                    Err(ServiceError::NoContentSource) => model.filters.loaded = true,
                    Err(error) => {
                        model.filters.loaded = true;
                        model.filters.error = Some(error.to_string());
                    }
                },
                cx,
            );
        },
    );
}

/// Opening Discover for the first time loads the filter choices and lists
/// what is popular.
pub(super) fn first_search(wiring: &Wiring, cx: &mut App) {
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let pending = wiring
            .shell
            .read_with(cx, |shell, _| {
                shell
                    .live()
                    .filter(|model| model.search == SearchStatus::Idle)
                    .map(|model| model.query.clone())
            })
            .ok()
            .flatten();
        let Some(query) = pending else { return };
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.update_live(|model| model.search = SearchStatus::Searching, cx);
        });
        cx.update(|cx| {
            load_filters(&wiring, cx);
            search(&wiring, &query, cx);
        });
    })
    .detach();
}

/// Installs a project. `version` picks one file of a mod, pack or shader.
pub(super) fn install(
    wiring: &Wiring,
    kind: ProjectKind,
    slug: String,
    title: String,
    version: Option<String>,
    window: &mut Window,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let cancel = CancellationToken::new();
    let source = source_of(wiring, &slug, cx);
    if kind == ProjectKind::Modpack {
        mark_installing(wiring, &slug, true, cx);
        let slug_done = slug.clone();
        job(
            wiring,
            cx,
            true,
            Reload::All,
            Some(Toast::info(tr!(
                "discover-install-started",
                title = title.as_str()
            ))),
            async move {
                service
                    .from_content_source(source, service.install_modpack(&slug, cancel))
                    .await
            },
            move |shell, result, cx| {
                mark_installing_in(shell, &slug_done, false, cx);
                let toast = outcome(
                    result,
                    |record| tr!("discover-modpack-installed", name = record.name),
                    tr!("discover-install-failed", title = title.as_str()),
                );
                shell.toast(toast, cx);
            },
        );
        return;
    }
    let target = install_target(wiring, cx);
    let Some((instance, instance_name)) = target else {
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.toast(Toast::info(tr!("discover-needs-game")), cx);
        });
        return;
    };
    if kind != ProjectKind::Mod {
        run_install(
            wiring,
            instance,
            instance_name,
            kind,
            slug,
            title,
            version,
            Vec::new(),
            source,
            cx,
        );
        return;
    }
    // A mod may need other mods. Not being able to find out must not stop
    // the install, so a failed check installs the mod alone.
    let check = {
        let (instance, slug, version) = (instance.clone(), slug.clone(), version.clone());
        let source = source.clone();
        wiring.backend.spawn(async move {
            service
                .from_content_source(
                    source,
                    service.dependency_report(
                        &instance,
                        ProjectKind::Mod,
                        &slug,
                        version.as_deref(),
                    ),
                )
                .await
        })
    };
    let wiring = wiring.clone();
    let window_handle = window.window_handle();
    cx.spawn(async move |cx| {
        let report = check.await.ok().and_then(Result::ok).unwrap_or_default();
        cx.update(|cx| {
            if !DependencyPrompt::worth_showing(&report) {
                run_install(
                    &wiring,
                    instance,
                    instance_name,
                    kind,
                    slug,
                    title,
                    version,
                    Vec::new(),
                    source.clone(),
                    cx,
                );
                return;
            }
            let _ = cx.update_window(window_handle, |_, window, cx| {
                let form = cx.new(|_| {
                    let (job_wiring, rows) = (
                        wiring.clone(),
                        report
                            .needs
                            .iter()
                            .chain(report.optional.iter())
                            .cloned()
                            .collect::<Vec<_>>(),
                    );
                    let (instance, instance_name) = (instance.clone(), instance_name.clone());
                    let (slug, title, version) = (slug.clone(), title.clone(), version.clone());
                    DependencyPrompt::new(
                        title.clone(),
                        report,
                        Rc::new(move |chosen, _, cx| {
                            // Each chosen dependency, at the version the check found.
                            let extra: Vec<(String, String, String)> = chosen
                                .iter()
                                .filter_map(|id| {
                                    let need = rows.iter().find(|need| &need.project_id == id)?;
                                    let found = need.version.as_ref()?;
                                    Some((id.clone(), found.id.clone(), need.title.clone()))
                                })
                                .collect();
                            run_install(
                                &job_wiring,
                                instance.clone(),
                                instance_name.clone(),
                                kind,
                                slug.clone(),
                                title.clone(),
                                version.clone(),
                                extra,
                                source.clone(),
                                cx,
                            );
                        }),
                    )
                });
                DependencyPrompt::open(form, window, cx);
            });
        });
    })
    .detach();
}

/// Installs `slug` into the game, after any chosen dependencies (project,
/// version, title), each as its own task in Activity. A dependency that fails
/// is told, and the mod itself is still installed.
#[allow(clippy::too_many_arguments)]
pub(super) fn run_install(
    wiring: &Wiring,
    instance: String,
    instance_name: String,
    kind: ProjectKind,
    slug: String,
    title: String,
    version: Option<String>,
    dependencies: Vec<(String, String, String)>,
    source: Option<String>,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let state = wiring.state.clone();
    mark_installing(wiring, &slug, true, cx);
    let slug_done = slug.clone();
    job(
        wiring,
        cx,
        true,
        Reload::Lists,
        Some(Toast::info(tr!(
            "discover-install-started",
            title = title.as_str()
        ))),
        async move {
            service
                .from_content_source(source, async {
                    let mut failed = Vec::new();
                    for (project, version_id, name) in dependencies {
                        if service
                            .install_version(
                                &instance,
                                kind,
                                &project,
                                &version_id,
                                CancellationToken::new(),
                            )
                            .await
                            .is_err()
                        {
                            failed.push(name);
                        }
                    }
                    let installed = match version {
                        Some(version) => {
                            service
                                .install_version(
                                    &instance,
                                    kind,
                                    &slug,
                                    &version,
                                    CancellationToken::new(),
                                )
                                .await
                        }
                        None => {
                            service
                                .install_content(&instance, kind, &slug, CancellationToken::new())
                                .await
                        }
                    };
                    (installed, failed)
                })
                .await
        },
        move |shell, (result, failed), cx| {
            mark_installing_in(shell, &slug_done, false, cx);
            state.borrow_mut().installed_key = None;
            let installed = result.is_ok();
            let mut toast = outcome(
                result,
                |file| {
                    tr!(
                        "discover-installed-into",
                        file = file,
                        game = instance_name.as_str()
                    )
                },
                tr!("discover-install-failed", title = title.as_str()),
            );
            // A failed install already says so; a dependency that failed
            // under a successful one is the news.
            if installed && !failed.is_empty() {
                toast = Toast::error(tr!(
                    "discover-dependencies-failed",
                    dependencies = failed.join(tr!("common-list-separator")),
                    title = title.as_str()
                ));
            }
            shell.toast(toast, cx);
        },
    );
}

/// Marks a project as being installed (its card says so and cannot be
/// pressed again) or done.
fn mark_installing(wiring: &Wiring, slug: &str, on: bool, cx: &mut App) {
    let _ = wiring
        .shell
        .update(cx, |shell, cx| mark_installing_in(shell, slug, on, cx));
}

fn mark_installing_in(
    shell: &mut LauncherShell,
    slug: &str,
    on: bool,
    cx: &mut gpui_kit::Context<LauncherShell>,
) {
    shell.update_live(
        |model| {
            if on {
                model.installing.insert(slug.to_owned());
            } else {
                model.installing.remove(slug);
            }
        },
        cx,
    );
}

/// The instance installs go into: its id and name.
pub(super) fn install_target(wiring: &Wiring, cx: &mut App) -> Option<(String, String)> {
    wiring
        .shell
        .read_with(cx, |shell, _| {
            shell.live().and_then(|model| {
                let id = model.install_target.clone()?;
                let name = model
                    .library
                    .iter()
                    .find(|card| card.id == id)
                    .map(|card| card.name.clone())?;
                Some((id, name))
            })
        })
        .ok()
        .flatten()
}

/// A project's detail, in place of the Discover list: it appears at once and
/// fills in when the data arrives.
pub(super) fn open_project(
    wiring: &Wiring,
    kind: ProjectKind,
    slug: String,
    window: &mut Window,
    cx: &mut App,
) {
    let wiring = wiring.clone();
    // Deferred: this may be running inside one of the shell's own updates.
    window.defer(cx, move |_, cx| {
        let project = slug.clone();
        let _ = wiring
            .shell
            .update(cx, |shell, cx| shell.open_detail(kind, &project, cx));
        let service = wiring.backend.service.clone();
        let source = wiring
            .shell
            .read_with(cx, |shell, _| {
                shell.live().and_then(|model| {
                    model
                        .results
                        .iter()
                        .find(|row| row.slug == slug)
                        .map(|row| row.source.clone())
                })
            })
            .ok()
            .flatten();
        let handle = wiring.backend.spawn(async move {
            match source {
                Some(source) if !source.is_empty() => {
                    service.project_detail_from(&source, &slug).await
                }
                _ => service.project_detail(&slug).await,
            }
        });
        let shell = wiring.shell.clone();
        let project = slug_of(&shell, cx);
        let opened_view = shell
            .read_with(cx, |shell, _| shell.detail_view())
            .ok()
            .flatten();
        cx.spawn(async move |cx| {
            let Ok(result) = handle.await else { return };
            let state = match result {
                Ok(detail) => DetailState::Ready(Box::new(detail)),
                Err(error) => DetailState::Failed(error.to_string()),
            };
            if let Some(project) = project {
                let _ = shell.update(cx, |shell, cx| {
                    if shell.detail_view() != opened_view {
                        return;
                    }
                    shell.set_detail_state(&project, state, cx);
                    // Dev hook: `LUMILIO_DETAIL_TAB=1|2` starts on Versions / Gallery.
                    if let Some(tab) = std::env::var("LUMILIO_DETAIL_TAB")
                        .ok()
                        .and_then(|tab| tab.parse::<usize>().ok())
                    {
                        shell.select_detail_tab(tab, cx);
                    }
                });
            }
            cx.update(|cx| request_detail_summary(&wiring, cx));
        })
        .detach();
    });
}

/// The slug of the project the shell currently shows.
pub(super) fn slug_of(shell: &WeakEntity<LauncherShell>, cx: &mut App) -> Option<String> {
    shell
        .read_with(cx, |shell, _| {
            shell.detail_project().map(|(_, slug)| slug.to_owned())
        })
        .ok()
        .flatten()
}

pub(super) fn source_of(wiring: &Wiring, project: &str, cx: &mut App) -> Option<String> {
    wiring
        .shell
        .read_with(cx, |shell, cx| {
            if let Some(view) = shell.detail_view()
                && let DetailState::Ready(detail) = view.read(cx).state()
                && (detail.project.slug == project || detail.project.id == project)
            {
                return Some(detail.project.source.clone());
            }
            shell
                .live()?
                .results
                .iter()
                .find(|row| row.slug == project || row.project_id == project)
                .map(|row| row.source.clone())
        })
        .ok()
        .flatten()
        .filter(|source| !source.is_empty())
}

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
    let Some((view, (source, project, original, revision))) = pending else {
        return;
    };
    let service = wiring.backend.service.clone();
    let (asked_source, asked_project, asked_original) =
        (source.clone(), project.clone(), original.clone());
    let handle = wiring.backend.spawn(async move {
        service
            .translate_summaries(&asked_source, &[(asked_project.clone(), asked_original)])
            .await
            .remove(&asked_project)
    });
    let shell = wiring.shell.clone();
    cx.spawn(async move |cx| {
        let Ok(translated) = handle.await else { return };
        let current = shell
            .read_with(cx, |shell, _| shell.detail_view())
            .ok()
            .flatten();
        if current != Some(view.clone()) {
            return;
        }
        view.update(cx, |view, cx| {
            view.set_summary_translation(revision, &source, &project, &original, translated, cx)
        });
    })
    .detach();
}

fn request_summaries(wiring: &Wiring, seq: u64, cx: &mut gpui_kit::Context<LauncherShell>) {
    let wiring = wiring.clone();
    cx.spawn(async move |_, cx| {
        cx.update(|cx| {
            let rows = wiring
                .shell
                .read_with(cx, |shell, _| {
                    shell.live().map(|model| model.results.clone())
                })
                .ok()
                .flatten()
                .unwrap_or_default();
            let Some(source) = rows.first().map(|row| row.source.clone()) else {
                return;
            };
            let items = rows
                .iter()
                .map(|row| (row.project_id.clone(), row.original_summary.clone()))
                .collect::<Vec<_>>();
            let service = wiring.backend.service.clone();
            let state = wiring.state.clone();
            let asked_source = source.clone();
            job(
                &wiring,
                cx,
                false,
                Reload::Nothing,
                None,
                async move { service.translate_summaries(&source, &items).await },
                move |shell, translations, cx| {
                    if state.borrow().search_seq != seq {
                        return;
                    }
                    shell.update_live(
                        |model| {
                            if model
                                .settings
                                .as_ref()
                                .is_some_and(|settings| settings.preferences.translation.enabled)
                            {
                                for row in &mut model.results {
                                    if row.source == asked_source
                                        && let Some(text) = translations.get(&row.project_id)
                                    {
                                        row.summary.clone_from(text);
                                    }
                                }
                            }
                        },
                        cx,
                    );
                },
            );
        });
    })
    .detach();
}

pub(super) fn translate_body(
    wiring: &Wiring,
    source: String,
    project: String,
    original: String,
    request: u64,
    cx: &mut App,
) {
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let view = cx.update(|cx| {
            wiring
                .shell
                .read_with(cx, |shell, _| shell.detail_view())
                .ok()
                .flatten()
        });
        let Some(view) = view else { return };
        let service = wiring.backend.service.clone();
        let handle = wiring
            .backend
            .spawn(async move { service.translate_body(&source, &project, &original).await });
        while !handle.is_finished() {
            cx.background_executor().timer(super::POLL).await;
            let current = cx.update(|cx| {
                wiring
                    .shell
                    .read_with(cx, |shell, _| shell.detail_view())
                    .ok()
                    .flatten()
                    == Some(view.clone())
                    && view.read(cx).translation_request() == Some(request)
            });
            if !current {
                handle.abort();
                return;
            }
        }
        if let Ok(result) = handle.await {
            view.update(cx, |view, cx| view.set_translation(request, result, cx));
        }
    })
    .detach();
}
