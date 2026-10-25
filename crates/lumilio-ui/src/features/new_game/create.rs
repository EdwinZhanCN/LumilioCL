use std::path::PathBuf;
use std::rc::Rc;

use gpui::{AnyWindowHandle, App, AppContext as _, Context, EventEmitter, WeakEntity, Window};
use lumilio_core::{CancellationToken, LaunchServiceError, ServiceError};

use super::form::{NewGameForm, NewGameIntent, NewGameRequest};
use crate::live::loader_label;
use crate::runtime::{self, Worker};
use crate::toast::{self, Toast, install_outcome};
use crate::tr;

/// What making games tells the application.
#[derive(Clone, Debug)]
pub enum NewGameEvent {
    /// Work started here changed the library or the tasks Activity lists.
    TasksChanged,
    /// A game was made in `window`; the application shows it there and makes
    /// it the current game.
    Created { id: String, window: AnyWindowHandle },
    /// A new game's download ended; an open page for it reads it again.
    Installed(String),
}

/// Makes new games: from the new-game dialog, or from a pack.
pub struct NewGame {
    worker: Option<Worker>,
}

impl EventEmitter<NewGameEvent> for NewGame {}

impl NewGame {
    #[must_use]
    pub fn new(worker: Option<Worker>) -> Self {
        Self { worker }
    }

    /// Gives the feature the launcher to ask (the application does, once).
    pub fn set_worker(&mut self, worker: Worker) {
        self.worker = Some(worker);
    }

    /// Opens the new-game dialog (IA `library.md#新建游戏`). Deferred: this
    /// may run inside one of the shell's own updates.
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(worker) = self.worker.clone() else {
            return;
        };
        let this = cx.weak_entity();
        window.defer(cx, move |window, cx| {
            let handle = window.window_handle();
            let form = cx.new(|cx| {
                let form = cx.weak_entity();
                NewGameForm::new(
                    Rc::new(move |intent, _, cx| match intent {
                        NewGameIntent::Create(request) => {
                            let form = form.clone();
                            let _ = this.update(cx, |this, cx| {
                                this.create(form, handle, request, cx);
                            });
                        }
                        other => answer_versions(&worker, form.clone(), other, cx),
                    }),
                    window,
                    cx,
                )
            });
            NewGameForm::open(form, window, cx);
        });
    }

    /// Asks for a local pack (`.mrpack`, a CurseForge or MultiMC / Prism zip,
    /// a backup) and imports it as a new game. Cancelling the picker does
    /// nothing.
    pub fn import_pack(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr!("new-game-import-pack-prompt").into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update(cx, |this, cx| this.import_file(path, cx));
        })
        .detach();
    }

    /// Files dropped on the Library: each pack becomes a new game, and
    /// anything else is said to be unsupported.
    pub fn import_files(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let packs: Vec<_> = paths
            .into_iter()
            .filter(|path| {
                path.extension().is_some_and(|ext| {
                    ext.eq_ignore_ascii_case("mrpack") || ext.eq_ignore_ascii_case("zip")
                })
            })
            .collect();
        if packs.is_empty() {
            toast::post(Toast::info(tr!("library-drop-unsupported")), cx);
        }
        for path in packs {
            self.import_file(path, cx);
        }
    }

    /// Creates the game, then (if asked) downloads its files in the
    /// background; the application shows it and makes it the current game.
    pub(super) fn create(
        &mut self,
        form: WeakEntity<NewGameForm>,
        window: AnyWindowHandle,
        request: NewGameRequest,
        cx: &mut Context<Self>,
    ) {
        let Some(worker) = &self.worker else {
            return;
        };
        let service = worker.service().clone();
        let wanted = request.clone();
        let handle = worker.spawn(async move {
            service
                .create_instance(
                    &wanted.name,
                    Some(&wanted.game_version),
                    wanted.loader,
                    wanted.loader_version.as_deref(),
                )
                .await
        });
        cx.spawn(async move |this, cx| {
            let record = match handle.await.unwrap_or_else(|error| Err(error.into())) {
                Ok(record) => record,
                Err(error) => {
                    let message = match &error {
                        ServiceError::Launch(LaunchServiceError::LoaderUnsupported(loader)) => {
                            tr!(
                                "new-game-loader-unsupported-install",
                                loader = loader_label(*loader)
                            )
                        }
                        _ => tr!("new-game-create-failed").to_owned(),
                    };
                    let _ = form.update(cx, |form, cx| {
                        form.created(Err((message, error.to_string())), cx)
                    });
                    return;
                }
            };
            let _ = form.update(cx, |form, cx| form.created(Ok(()), cx));
            let _ = this.update(cx, |this, cx| {
                toast::post(
                    Toast::success(if request.install {
                        tr!("new-game-created-downloading", name = record.name)
                    } else {
                        tr!("new-game-created", name = record.name)
                    }),
                    cx,
                );
                cx.emit(NewGameEvent::TasksChanged);
                if request.install {
                    this.install(record.id.clone(), cx);
                }
                cx.emit(NewGameEvent::Created {
                    id: record.id,
                    window,
                });
            });
        })
        .detach();
    }

    /// Downloads a new game's files; progress lives in Activity.
    fn install(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(worker) = &self.worker else {
            return;
        };
        let service = worker.service().clone();
        let target = id.clone();
        runtime::job(
            worker,
            cx,
            Some(NewGameEvent::TasksChanged),
            None,
            async move {
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                tokio::spawn(async move { while rx.recv().await.is_some() {} });
                service
                    .install_instance(&id, tx, CancellationToken::new())
                    .await
            },
            move |_, result, cx| {
                if let Err(error) = result {
                    toast::post(
                        Toast::error(tr!("new-game-download-failed")).technical(error.to_string()),
                        cx,
                    );
                }
                cx.emit(NewGameEvent::Installed(target));
            },
        );
    }

    /// Imports a chosen or dropped pack as a new game.
    fn import_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let Some(worker) = &self.worker else {
            return;
        };
        let service = worker.service().clone();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        runtime::job(
            worker,
            cx,
            Some(NewGameEvent::TasksChanged),
            Some(Toast::info(tr!(
                "library-import-game-started",
                name = name.as_str()
            ))),
            async move {
                let record = service
                    .import_modpack_file(&path, CancellationToken::new())
                    .await?;
                // Files the launcher may not download are listed on the game.
                let missing = service
                    .pending_downloads(&record.id)
                    .await
                    .map(|pending| pending.len())
                    .unwrap_or_default();
                Ok::<_, ServiceError>((record, missing))
            },
            move |_, result, cx| {
                let toast = match result {
                    Ok((record, missing)) if missing > 0 => Toast::info(tr!(
                        "discover-modpack-missing",
                        name = record.name,
                        count = missing
                    ))
                    .sticky(),
                    other => install_outcome(
                        other,
                        |(record, _)| tr!("new-game-import-pack-done", name = record.name),
                        tr!("library-import-game-failed", name = name.as_str()),
                    ),
                };
                toast::post(toast, cx);
            },
        );
    }
}

/// Answers the dialog's requests for version lists. A creation is the
/// caller's to answer: it decides what one means (a new game, or another
/// version for an existing one).
pub fn answer_versions(
    worker: &Worker,
    form: WeakEntity<NewGameForm>,
    intent: NewGameIntent,
    cx: &mut App,
) {
    let service = worker.service().clone();
    match intent {
        NewGameIntent::LoadGameVersions => {
            let handle = worker.spawn(async move { service.game_versions().await });
            cx.spawn(async move |cx| {
                let result = match handle.await {
                    Ok(Ok(entries)) => Ok(entries),
                    Ok(Err(error)) => Err(error.to_string()),
                    Err(error) => Err(error.to_string()),
                }
                .map_err(|technical| (tr!("new-game-read-game-versions").to_owned(), technical));
                let _ = form.update(cx, |form, cx| form.game_versions_arrived(result, cx));
            })
            .detach();
        }
        NewGameIntent::LoadLoaderVersions {
            loader,
            game_version,
        } => {
            let wanted = game_version.clone();
            let handle =
                worker.spawn(async move { service.loader_versions(loader, &wanted).await });
            cx.spawn(async move |cx| {
                let result = match handle.await {
                    Ok(Ok(versions)) => Ok(versions),
                    Ok(Err(error)) => Err(error.to_string()),
                    Err(error) => Err(error.to_string()),
                }
                .map_err(|technical| {
                    let failed = tr!(
                        "new-game-read-loader-versions",
                        loader = loader_label(loader)
                    );
                    (failed, technical)
                });
                let _ = form.update(cx, |form, cx| {
                    form.loader_versions_arrived(loader, &game_version, result, cx)
                });
            })
            .detach();
        }
        NewGameIntent::Create(_) => {}
    }
}
