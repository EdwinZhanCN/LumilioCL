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
    if kind == ProjectKind::Modpack {
        mark_installing(wiring, &slug, true, cx);
        let slug_done = slug.clone();
        job(
            wiring,
            cx,
            true,
            Reload::All,
            Some(Toast::info(format!("开始安装 {title}，进度在动态里"))),
            async move { service.install_modpack(&slug, cancel).await },
            move |shell, result, cx| {
                mark_installing_in(shell, &slug_done, false, cx);
                let toast = outcome(
                    result,
                    |record| format!("已安装整合包 {}", record.name),
                    format!("没有装上 {title}"),
                );
                shell.toast(toast, cx);
            },
        );
        return;
    }
    let target = install_target(wiring, cx);
    let Some((instance, instance_name)) = target else {
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.toast(Toast::info("先在游戏库里新建一个游戏"), cx);
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
            cx,
        );
        return;
    }
    // A mod may need other mods. Not being able to find out must not stop
    // the install, so a failed check installs the mod alone.
    let check = {
        let (instance, slug, version) = (instance.clone(), slug.clone(), version.clone());
        wiring.backend.spawn(async move {
            service
                .dependency_report(&instance, ProjectKind::Mod, &slug, version.as_deref())
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
        Some(Toast::info(format!("开始安装 {title}，进度在动态里"))),
        async move {
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
                        .install_version(&instance, kind, &slug, &version, CancellationToken::new())
                        .await
                }
                None => {
                    service
                        .install_content(&instance, kind, &slug, CancellationToken::new())
                        .await
                }
            };
            (installed, failed)
        },
        move |shell, (result, failed), cx| {
            mark_installing_in(shell, &slug_done, false, cx);
            state.borrow_mut().installed_key = None;
            let installed = result.is_ok();
            let mut toast = outcome(
                result,
                |file| format!("已把 {file} 装进 {instance_name}"),
                format!("没有装上 {title}"),
            );
            // A failed install already says so; a dependency that failed
            // under a successful one is the news.
            if installed && !failed.is_empty() {
                toast = Toast::error(format!(
                    "{} 没有装上，{title} 可能进不了游戏",
                    failed.join("、")
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
        let handle = wiring
            .backend
            .spawn(async move { service.project_detail(&slug).await });
        let shell = wiring.shell.clone();
        let project = slug_of(&shell, cx);
        cx.spawn(async move |cx| {
            let Ok(result) = handle.await else { return };
            let state = match result {
                Ok(detail) => DetailState::Ready(Box::new(detail)),
                Err(error) => DetailState::Failed(error.to_string()),
            };
            if let Some(project) = project {
                let _ = shell.update(cx, |shell, cx| {
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
