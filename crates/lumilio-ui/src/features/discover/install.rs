use std::rc::Rc;

use gpui::prelude::*;
use gpui::{Context, Window};
use lumilio_core::{CancellationToken, ProjectKind};

use super::dependencies::DependencyPrompt;
use super::detail::{DetailHandler, DetailIntent, DetailState, ProjectDetailView};
use super::page::{Discover, DiscoverEvent, OpenedProject};
use super::view::DiscoverIntent;
use crate::runtime::answer;
use crate::toast::{Toast, install_outcome};
use crate::tr;

impl Discover {
    /// What the page's rows and menus ask for.
    pub(super) fn act(
        &mut self,
        intent: DiscoverIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(test)]
        self.acted.push(intent.clone());
        match intent {
            DiscoverIntent::SearchAgain => self.search_again(cx),
            DiscoverIntent::OpenProject { kind, source, slug } => {
                let opened = self.open_project(kind, source, slug, cx);
                cx.emit(DiscoverEvent::ProjectOpened(opened));
            }
            DiscoverIntent::Install {
                kind,
                source,
                slug,
                title,
                version,
            } => self.install(kind, source, slug, title, version, window, cx),
            DiscoverIntent::UpdateInstalled {
                kind,
                source,
                project,
                title,
                file_name,
                version_id,
            } => self.update_installed(kind, source, project, title, file_name, version_id, cx),
            DiscoverIntent::CopyText { text, notice } => {
                crate::toast::copy_text(text, cx);
                crate::toast::post(Toast::success(notice), cx);
            }
        }
    }

    /// A project's detail, for showing in place of the list: it appears at
    /// once and fills in when the data arrives.
    pub fn open_project(
        &mut self,
        kind: ProjectKind,
        source: String,
        slug: String,
        cx: &mut Context<Self>,
    ) -> OpenedProject {
        let page = cx.entity().downgrade();
        let handler: DetailHandler = {
            let (source, project) = (source.clone(), slug.clone());
            Rc::new(move |intent, window, cx| {
                let (source, project) = (source.clone(), project.clone());
                let _ = page.update(cx, |page, cx| match intent {
                    DetailIntent::SaveAs {
                        version_id,
                        file_name,
                        title,
                    } => page.save_version_as(source, project, version_id, file_name, title, cx),
                    DetailIntent::Update {
                        file_name,
                        version_id,
                        title,
                    } => page
                        .update_installed(kind, source, project, title, file_name, version_id, cx),
                    DetailIntent::Install { version_id, title } => {
                        page.install(kind, source, project, title, version_id, window, cx)
                    }
                    DetailIntent::Translate {
                        source,
                        project,
                        original,
                        format,
                        request,
                    } => page.translate_body(source, project, original, format, request, cx),
                });
            })
        };
        let target = self.detail_target();
        let tags = self.state.filters.game_tags().to_vec();
        let translation = self.state.translation;
        // The page's address comes with the project.
        let view = cx.new(|_| {
            let mut view = ProjectDetailView::new(slug.clone(), String::new(), handler);
            view.set_target_quiet(target);
            view
        });
        view.update(cx, |view, cx| {
            view.set_game_tags(tags, cx);
            view.set_translation_enabled(translation, cx);
        });
        self.details.push((view.downgrade(), slug.clone()));
        let opened = OpenedProject {
            view: view.clone(),
            kind,
            source: source.clone(),
            slug: slug.clone(),
        };
        let Some(worker) = self.worker.clone() else {
            return opened;
        };
        let service = worker.service().clone();
        let handle = worker.spawn(async move { service.project_detail(&source, &slug).await });
        let detail = view.downgrade();
        cx.spawn(async move |page, cx| {
            let result = handle.await.unwrap_or_else(|error| Err(error.into()));
            let state = match result {
                Ok(detail) => DetailState::Ready(Box::new(detail)),
                Err(error) => DetailState::Failed(error.to_string()),
            };
            let Some(view) = detail.upgrade() else {
                return;
            };
            view.update(cx, |view, cx| {
                view.set_state(state, cx);
                // Dev hook: `LUMILIO_DETAIL_TAB=1|2` starts on Versions / Gallery.
                if let Some(tab) = std::env::var("LUMILIO_DETAIL_TAB")
                    .ok()
                    .and_then(|tab| tab.parse::<usize>().ok())
                {
                    view.select_tab(tab, cx);
                }
            });
            let _ = page.update(cx, |page, cx| page.request_detail_summary(&view, cx));
        })
        .detach();
        opened
    }

    /// Installs a project. `version` picks one file of a mod, pack or shader.
    #[expect(clippy::too_many_arguments, reason = "one install's inputs")]
    pub(super) fn install(
        &mut self,
        kind: ProjectKind,
        source: String,
        slug: String,
        title: String,
        version: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(worker) = self.worker.clone() else {
            return;
        };
        let service = worker.service().clone();
        let cancel = CancellationToken::new();
        if kind == ProjectKind::Modpack {
            self.mark_installing(&slug, true, cx);
            let slug_done = slug.clone();
            self.job(
                cx,
                true,
                Some(Toast::info(tr!(
                    "discover-install-started",
                    title = title.as_str()
                ))),
                async move {
                    let record = service
                        .install_modpack(&source, &slug, version.as_deref(), cancel)
                        .await?;
                    // Files the launcher may not download are listed on the game.
                    let missing = service
                        .pending_downloads(&record.id)
                        .await
                        .map(|pending| pending.len())
                        .unwrap_or_default();
                    Ok((record, missing))
                },
                move |page, result, cx| {
                    page.mark_installing(&slug_done, false, cx);
                    let toast = match result {
                        Ok((record, missing)) if missing > 0 => Toast::info(tr!(
                            "discover-modpack-missing",
                            name = record.name,
                            count = missing
                        ))
                        .sticky(),
                        other => install_outcome(
                            other,
                            |(record, _)| tr!("discover-modpack-installed", name = record.name),
                            tr!("discover-install-failed", title = title.as_str()),
                        ),
                    };
                    crate::toast::post(toast, cx);
                },
            );
            return;
        }
        let Some((instance, instance_name)) = self
            .state
            .target_game()
            .map(|card| (card.id.clone(), card.name.clone()))
        else {
            crate::toast::post(Toast::info(tr!("discover-needs-game")), cx);
            return;
        };
        if kind != ProjectKind::Mod {
            self.run_install(
                instance,
                instance_name,
                kind,
                source,
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
            let (instance, source, slug, version) = (
                instance.clone(),
                source.clone(),
                slug.clone(),
                version.clone(),
            );
            worker.spawn(async move {
                service
                    .dependency_report(
                        &instance,
                        ProjectKind::Mod,
                        &source,
                        &slug,
                        version.as_deref(),
                    )
                    .await
            })
        };
        let window_handle = window.window_handle();
        cx.spawn(async move |page, cx| {
            let report = answer(check, cx)
                .await
                .and_then(Result::ok)
                .unwrap_or_default();
            if !DependencyPrompt::worth_showing(&report) {
                let _ = page.update(cx, |page, cx| {
                    page.run_install(
                        instance,
                        instance_name,
                        kind,
                        source,
                        slug,
                        title,
                        version,
                        Vec::new(),
                        cx,
                    );
                });
                return;
            }
            let _ = cx.update_window(window_handle, |_, window, cx| {
                let rows: Vec<_> = report
                    .needs
                    .iter()
                    .chain(report.optional.iter())
                    .cloned()
                    .collect();
                let form = cx.new(|_| {
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
                            let (instance, instance_name) =
                                (instance.clone(), instance_name.clone());
                            let (source, slug, title, version) =
                                (source.clone(), slug.clone(), title.clone(), version.clone());
                            let _ = page.update(cx, |page, cx| {
                                page.run_install(
                                    instance,
                                    instance_name,
                                    kind,
                                    source,
                                    slug,
                                    title,
                                    version,
                                    extra,
                                    cx,
                                );
                            });
                        }),
                    )
                });
                DependencyPrompt::open(form, window, cx);
            });
        })
        .detach();
    }

    /// Installs `slug` into the game, after any chosen dependencies (project,
    /// version, title; of the same source), each as its own task in Activity.
    /// A dependency that fails is told, and the mod itself is still installed.
    #[expect(clippy::too_many_arguments, reason = "one install's inputs")]
    fn run_install(
        &mut self,
        instance: String,
        instance_name: String,
        kind: ProjectKind,
        source: String,
        slug: String,
        title: String,
        version: Option<String>,
        dependencies: Vec<(String, String, String)>,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.worker.as_ref().map(|worker| worker.service().clone()) else {
            return;
        };
        self.mark_installing(&slug, true, cx);
        let slug_done = slug.clone();
        self.job(
            cx,
            true,
            Some(Toast::info(tr!(
                "discover-install-started",
                title = title.as_str()
            ))),
            async move {
                let mut failed = Vec::new();
                for (project, version_id, name) in dependencies {
                    if let Err(error) = service
                        .install_version(
                            &instance,
                            kind,
                            &source,
                            &project,
                            &version_id,
                            CancellationToken::new(),
                        )
                        .await
                    {
                        failed.push((name, error.to_string()));
                    }
                }
                let installed = match version {
                    Some(version) => {
                        service
                            .install_version(
                                &instance,
                                kind,
                                &source,
                                &slug,
                                &version,
                                CancellationToken::new(),
                            )
                            .await
                    }
                    None => {
                        service
                            .install_content(
                                &instance,
                                kind,
                                &source,
                                &slug,
                                CancellationToken::new(),
                            )
                            .await
                    }
                };
                // Under a failed install the dependencies are not the news.
                installed.map(|file| (file, failed))
            },
            move |page, result, cx| {
                page.mark_installing(&slug_done, false, cx);
                page.installed_key = None;
                let (result, failed) = match result {
                    Ok((file, failed)) => (Ok(file), failed),
                    Err(error) => (Err(error), Vec::new()),
                };
                let mut toast = install_outcome(
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
                if !failed.is_empty() {
                    let names: Vec<&str> = failed.iter().map(|(name, _)| name.as_str()).collect();
                    let causes: Vec<String> = failed
                        .iter()
                        .map(|(name, cause)| format!("{name}: {cause}"))
                        .collect();
                    toast = Toast::error(tr!(
                        "discover-dependencies-failed",
                        dependencies = names.join(tr!("common-list-separator")),
                        title = title.as_str()
                    ))
                    .technical(causes.join("\n"));
                }
                crate::toast::post(toast, cx);
            },
        );
    }

    /// Replaces an installed file with its newer version, in the game installs
    /// go into.
    #[expect(clippy::too_many_arguments, reason = "one update's inputs")]
    fn update_installed(
        &mut self,
        kind: ProjectKind,
        source: String,
        project: String,
        title: String,
        file_name: String,
        version_id: String,
        cx: &mut Context<Self>,
    ) {
        let Some((instance, instance_name)) = self
            .state
            .target_game()
            .map(|card| (card.id.clone(), card.name.clone()))
        else {
            return;
        };
        let Some(service) = self.worker.as_ref().map(|worker| worker.service().clone()) else {
            return;
        };
        self.job(
            cx,
            true,
            Some(Toast::info(tr!(
                "project-update-started",
                title = title.as_str()
            ))),
            async move {
                service
                    .switch_content_version(
                        &instance,
                        kind,
                        &file_name,
                        &source,
                        &project,
                        &version_id,
                        CancellationToken::new(),
                    )
                    .await
            },
            move |page, result, cx| {
                page.installed_key = None;
                crate::toast::post(
                    install_outcome(
                        result,
                        |file| {
                            tr!(
                                "project-updated-to",
                                file = file,
                                instance = instance_name.as_str()
                            )
                        },
                        tr!("project-update-failed", title = title.as_str()),
                    ),
                    cx,
                );
            },
        );
    }

    /// Downloads one version's file where the person chooses.
    fn save_version_as(
        &mut self,
        source: String,
        project: String,
        version_id: String,
        file_name: String,
        title: String,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.worker.as_ref().map(|worker| worker.service().clone()) else {
            return;
        };
        let start = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| service.layout().root().to_owned());
        let chosen = crate::platform::pick_save_path(cx, &start, &file_name);
        cx.spawn(async move |page, cx| {
            let Some(path) = chosen.await else {
                return;
            };
            let _ = page.update(cx, |page, cx| {
                let target = path.clone();
                page.job(
                    cx,
                    true,
                    Some(Toast::info(tr!(
                        "project-save-started",
                        title = title.as_str()
                    ))),
                    async move {
                        service
                            .save_version_as(
                                &source,
                                &project,
                                &version_id,
                                &target,
                                CancellationToken::new(),
                            )
                            .await
                    },
                    move |_, result, cx| {
                        crate::toast::post(
                            install_outcome(
                                result,
                                |_| tr!("project-saved-to", path = path.display().to_string()),
                                tr!("project-save-failed", title = title.as_str()),
                            ),
                            cx,
                        );
                    },
                );
            });
        })
        .detach();
    }

    /// Marks a project as being installed (its card says so and cannot be
    /// pressed again) or done.
    fn mark_installing(&mut self, slug: &str, on: bool, cx: &mut Context<Self>) {
        if on {
            self.state.installing.insert(slug.to_owned());
        } else {
            self.state.installing.remove(slug);
        }
        self.state_changed(cx);
    }
}
