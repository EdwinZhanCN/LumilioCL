use super::discover::open_project;
use super::instance_write::write_instance;
use super::jobs::{refresh_installed, reload};
use super::launch::play;
use super::library::choose_current;
use super::new_game::new_game_intent;
use super::{Reload, Wiring};
use gpui_kit::AppContext as _;
use gpui_kit::{App, Entity, WeakEntity, Window};
use lumilio_core::{AfterLaunch, ContentEffect, ContentResult, LaunchSignal, ServiceError};
use lumilio_ui::export_form::ExportForm;
use lumilio_ui::instance_detail::{Arrived, InstanceDetailView, InstanceIntent, Section};
use lumilio_ui::new_game::{NewGameForm, NewGameIntent, RuntimeChange};
use lumilio_ui::platform;
use lumilio_ui::route::Route;
use lumilio_ui::toast::Toast;
use std::rc::Rc;

/// Opens a game's page; `then` is something to ask of the page once it is up.
pub(super) fn open_instance(
    wiring: &Wiring,
    id: String,
    then: Option<InstanceIntent>,
    window: &mut Window,
    cx: &mut App,
) {
    let wiring = wiring.clone();
    window.defer(cx, move |window, cx| {
        let target = id.clone();
        let callbacks = wiring.clone();
        let Ok(view) = wiring.shell.update(cx, |shell, cx| {
            shell.open_live_instance(
                id,
                move |source| {
                    Rc::new(move |intent, window, cx| {
                        instance_intent(&callbacks, &target, source.clone(), intent, window, cx);
                    })
                },
                cx,
            )
        }) else {
            return;
        };
        let id = target_id(&view, cx);
        // A game already running has printed things the page should show.
        let running = wiring
            .state
            .borrow()
            .game_log
            .as_ref()
            .filter(|(running_id, _)| *running_id == id)
            .map(|(_, lines)| lines.iter().cloned().collect::<Vec<_>>());
        if let Some(lines) = running {
            view.update(cx, |view, cx| view.game_output(lines, true, cx));
        }
        load_instance(&wiring, id.clone(), view.downgrade(), cx);
        if let Some(intent) = then {
            instance_intent(&wiring, &id, view.downgrade(), intent, window, cx);
        }
    });
}

pub(super) fn target_id(view: &Entity<InstanceDetailView>, cx: &App) -> String {
    view.read(cx).id().to_owned()
}

pub(super) fn load_instance(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let handle = wiring.backend.spawn(async move {
        let record = service.instance(&id).await?;
        Ok::<_, ServiceError>((record, service.settings().await))
    });
    let memory = wiring
        .backend
        .spawn(async { lumilio_core::total_memory_mb() });
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let _ = view.update(cx, |view, cx| view.loaded(result, cx));
        if let Ok(total) = memory.await {
            let _ = view.update(cx, |view, cx| view.set_machine_memory(total, cx));
        }
    })
    .detach();
}

pub(super) fn instance_intent(
    wiring: &Wiring,
    id: &str,
    view: WeakEntity<InstanceDetailView>,
    intent: InstanceIntent,
    window: &mut Window,
    cx: &mut App,
) {
    match intent {
        InstanceIntent::Play => play(wiring, id.to_owned(), window, cx),
        InstanceIntent::OpenRuntimeChange => {
            open_runtime_change(wiring, id.to_owned(), view, window, cx)
        }
        InstanceIntent::PlayWorld(world) => {
            wiring.state.borrow_mut().next_world = Some(world);
            play(wiring, id.to_owned(), window, cx);
        }
        InstanceIntent::Reload => load_instance(wiring, id.to_owned(), view, cx),
        InstanceIntent::OpenCrash(file) => open_crash(wiring, id.to_owned(), view, file, cx),
        InstanceIntent::Load(section) => load_section(wiring, id.to_owned(), view, section, cx),
        InstanceIntent::OpenAccounts => {
            let shell = wiring.shell.clone();
            window.defer(cx, move |window, cx| {
                let _ = shell.update(cx, |shell, cx| shell.go_to(Route::Accounts, window, cx));
            });
        }
        question @ (InstanceIntent::AskCopy
        | InstanceIntent::AskDelete
        | InstanceIntent::Resolve(_)) => {
            let _ = view.update(cx, |view, cx| view.ask_later(question, cx));
        }
        InstanceIntent::SetCurrent => choose_current(wiring, id.to_owned(), cx),
        InstanceIntent::Stop => {
            // Stops the launch or the running game, if it is this one.
            let this_one = wiring
                .state
                .borrow()
                .game_log
                .as_ref()
                .is_some_and(|(running, _)| running == id);
            if this_one && let Some(cancel) = wiring.state.borrow().cancel.clone() {
                cancel.cancel();
            }
        }
        InstanceIntent::ExportLog(crash) => export_log(wiring, id.to_owned(), crash, cx),
        InstanceIntent::OpenFolder(folder) => open_folder(wiring, id.to_owned(), view, folder, cx),
        InstanceIntent::RevealPath(relative) => {
            // Only plain names below the game directory; never a way out of it.
            let plain = std::path::Path::new(&relative)
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)));
            if plain {
                cx.reveal_path(&wiring.backend.service.game_dir(id).join(relative));
            }
        }
        InstanceIntent::LoadVersions(project) => {
            let service = wiring.backend.service.clone();
            let wanted = project.clone();
            let handle = wiring
                .backend
                .spawn(async move { service.project_versions(&wanted).await });
            cx.spawn(async move |cx| {
                let result = match handle.await {
                    Ok(result) => result.map_err(|error| error.to_string()),
                    Err(error) => Err(error.to_string()),
                };
                let _ = view.update(cx, |view, cx| view.versions_arrived(&project, result, cx));
            })
            .detach();
        }
        InstanceIntent::RevealContent { kind, file_name } => {
            let folder = kind.install_folder().unwrap_or_default();
            let path = wiring
                .backend
                .service
                .game_dir(id)
                .join(folder)
                .join(file_name);
            cx.reveal_path(&path);
        }
        InstanceIntent::OpenProject { kind, slug } => open_project(wiring, kind, slug, window, cx),
        // ia[discover]: 锁定目标（从游戏页进入） | 游戏页内容标签「浏览 Mod / 资源包 / 光影」 | 发现页进入「为这个游戏浏览」：页头写明游戏，类型预选，版本和加载器筛选锁定；安装目标换成那个游戏（右下角芯片显示）；不改变当前游戏
        // ia[instance.content]: 浏览并安装 | L4a 主要 → 发现页（类型预选） | 安装完回到本页可见
        InstanceIntent::BrowseContent(kind) => {
            let shell = wiring.shell.clone();
            let target = id.to_owned();
            let wiring = wiring.clone();
            window.defer(cx, move |window, cx| {
                let _ = shell.update(cx, |shell, cx| shell.browse_for(target, kind, window, cx));
                refresh_installed(&wiring, cx);
            });
        }
        InstanceIntent::ImportContent(kind) => {
            let picked = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
                files: true,
                directories: false,
                multiple: true,
                prompt: Some("选择要添加的文件".into()),
            });
            let wiring = wiring.clone();
            let id = id.to_owned();
            let handle = window.window_handle();
            cx.spawn(async move |cx| {
                let Ok(Ok(Some(files))) = picked.await else {
                    return;
                };
                let _ = cx.update_window(handle, |_, window, cx| {
                    write_instance(
                        &wiring,
                        id,
                        view,
                        InstanceIntent::AddFiles { kind, files },
                        window,
                        cx,
                    );
                });
            })
            .detach();
        }
        InstanceIntent::ExportWorld(folder) => {
            let start = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| wiring.backend.service.layout().root().to_owned());
            let chosen = platform::pick_save_path(cx, &start, &format!("{folder}.zip"));
            let wiring = wiring.clone();
            let id = id.to_owned();
            let handle = window.window_handle();
            cx.spawn(async move |cx| {
                let Some(path) = chosen.await else {
                    return;
                };
                let _ = cx.update_window(handle, |_, window, cx| {
                    write_instance(
                        &wiring,
                        id,
                        view,
                        InstanceIntent::ExportWorldTo { folder, path },
                        window,
                        cx,
                    );
                });
            })
            .detach();
        }
        InstanceIntent::BackupGame => {
            let start = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| wiring.backend.service.layout().root().to_owned());
            let suggested = format!("{}.lumilio-backup.zip", id).replace('/', "-");
            let chosen = platform::pick_save_path(cx, &start, &suggested);
            let wiring = wiring.clone();
            let id = id.to_owned();
            let handle = window.window_handle();
            cx.spawn(async move |cx| {
                let Some(path) = chosen.await else {
                    return;
                };
                let _ = cx.update_window(handle, |_, window, cx| {
                    write_instance(
                        &wiring,
                        id,
                        view,
                        InstanceIntent::BackupGameTo(path),
                        window,
                        cx,
                    );
                });
            })
            .detach();
        }
        InstanceIntent::ExportPack => open_export(wiring, id.to_owned(), view, window, cx),
        InstanceIntent::ExportPackAs(spec) => {
            let start = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| wiring.backend.service.layout().root().to_owned());
            let suggested = format!("{}-{}.{}", spec.name, spec.version, spec.format.extension())
                .replace('/', "-");
            let chosen = platform::pick_save_path(cx, &start, &suggested);
            let wiring = wiring.clone();
            let id = id.to_owned();
            let handle = window.window_handle();
            cx.spawn(async move |cx| {
                let Some(path) = chosen.await else {
                    return;
                };
                let _ = cx.update_window(handle, |_, window, cx| {
                    write_instance(
                        &wiring,
                        id,
                        view,
                        InstanceIntent::ExportPackTo { spec, path },
                        window,
                        cx,
                    );
                });
            })
            .detach();
        }
        InstanceIntent::ImportWorld => {
            let picked = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some("选择世界的 .zip".into()),
            });
            let wiring = wiring.clone();
            let id = id.to_owned();
            let handle = window.window_handle();
            cx.spawn(async move |cx| {
                let Ok(Ok(Some(files))) = picked.await else {
                    return;
                };
                let Some(path) = files.into_iter().next() else {
                    return;
                };
                let _ = cx.update_window(handle, |_, window, cx| {
                    write_instance(
                        &wiring,
                        id,
                        view,
                        InstanceIntent::AddWorld(path),
                        window,
                        cx,
                    );
                });
            })
            .detach();
        }
        InstanceIntent::Install
        | InstanceIntent::Repair
        | InstanceIntent::ChangeRuntime { .. }
        | InstanceIntent::Copy { .. }
        | InstanceIntent::Delete
        | InstanceIntent::SetContent { .. }
        | InstanceIntent::DeleteContent { .. }
        | InstanceIntent::CopyWorld(_)
        | InstanceIntent::BackupWorld(_)
        | InstanceIntent::ExportWorldTo { .. }
        | InstanceIntent::ExportPackTo { .. }
        | InstanceIntent::AddWorld(_)
        | InstanceIntent::DeleteWorld(_)
        | InstanceIntent::InstallJava(_)
        | InstanceIntent::BackupGameTo(_)
        | InstanceIntent::CreateSnapshot
        | InstanceIntent::CreateSnapshotAs { .. }
        | InstanceIntent::RestoreSnapshot(_)
        | InstanceIntent::DeleteSnapshot(_)
        | InstanceIntent::SwitchContent { .. }
        | InstanceIntent::UpdateContent { .. }
        | InstanceIntent::AddFiles { .. } => {
            write_instance(wiring, id.to_owned(), view, intent, window, cx)
        }
        action @ (InstanceIntent::Rename(_)
        | InstanceIntent::SaveMemory(_)
        | InstanceIntent::SaveSettings(_)) => {
            let service = wiring.backend.service.clone();
            let id = id.to_owned();
            let memory = matches!(action, InstanceIntent::SaveMemory(_));
            let settings_dialog = matches!(action, InstanceIntent::SaveSettings(_));
            let handle = wiring.backend.spawn(async move {
                match action {
                    InstanceIntent::Rename(name) => service.rename(&id, &name).await?,
                    InstanceIntent::SaveMemory(settings)
                    | InstanceIntent::SaveSettings(settings) => {
                        service.update_instance_settings(&id, settings).await?
                    }
                    _ => unreachable!("only save intents reach this branch"),
                }
                Ok::<_, ServiceError>((service.instance(&id).await?, service.settings().await))
            });
            let wiring = wiring.clone();
            cx.spawn(async move |cx| {
                let result = match handle.await {
                    Ok(result) => result.map_err(|error| error.to_string()),
                    Err(error) => Err(error.to_string()),
                };
                let _ = view.update(cx, |view, cx| {
                    if settings_dialog {
                        view.settings_saved(result, cx);
                    } else {
                        view.saved(memory, result, cx);
                    }
                });
                cx.update(|cx| reload(&wiring, Reload::All, cx));
            })
            .detach();
        }
    }
}

/// Reads one section's data off the interface thread.
pub(super) fn load_section(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    section: Section,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let handle = wiring.backend.spawn(async move {
        match section {
            Section::Content(kind) => Arrived::Content(
                kind,
                service
                    .content_details(&id, kind)
                    .await
                    .map_err(|error| error.to_string()),
            ),
            Section::Worlds => {
                Arrived::Worlds(service.worlds(&id).await.map_err(|error| error.to_string()))
            }
            Section::Snapshots => Arrived::Snapshots(
                service
                    .snapshots(&id)
                    .await
                    .map_err(|error| error.to_string()),
            ),
            Section::History => Arrived::History(
                service
                    .history(&id)
                    .await
                    .map_err(|error| error.to_string()),
            ),
            Section::Logs => {
                Arrived::Logs(service.logs(&id).await.map_err(|error| error.to_string()))
            }
            Section::Problems => Arrived::Problems(
                service
                    .problems(&id)
                    .await
                    .map_err(|error| error.to_string()),
            ),
            Section::Files => files_arrived(&service, &id, String::new()).await,
            Section::Size => Arrived::Size(
                service
                    .instance_size(&id)
                    .await
                    .map_err(|error| error.to_string()),
            ),
        }
    });
    cx.spawn(async move |cx| {
        let arrived = match handle.await {
            Ok(arrived) => arrived,
            Err(error) => failed_section(section, error.to_string()),
        };
        let _ = view.update(cx, |view, cx| view.arrived(arrived, cx));
    })
    .detach();
}

pub(super) fn open_crash(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    file: String,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let name = file.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.crash_report(&id, &name).await });
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let _ = view.update(cx, |view, cx| view.crash_arrived(file, result, cx));
    })
    .detach();
}

pub(super) fn failed_section(section: Section, detail: String) -> Arrived {
    match section {
        Section::Content(kind) => Arrived::Content(kind, Err(detail)),
        Section::Worlds => Arrived::Worlds(Err(detail)),
        Section::Snapshots => Arrived::Snapshots(Err(detail)),
        Section::History => Arrived::History(Err(detail)),
        Section::Logs => Arrived::Logs(Err(detail)),
        Section::Problems => Arrived::Problems(Err(detail)),
        Section::Files => Arrived::Files(String::new(), Err(detail)),
        Section::Size => Arrived::Size(Err(detail)),
    }
}

/// One folder of the game directory, as the Files part shows it.
pub(super) async fn files_arrived(
    service: &crate::backend::Service,
    id: &str,
    folder: String,
) -> Arrived {
    let result = service
        .list_files(id, &folder)
        .await
        .map_err(|error| error.to_string());
    Arrived::Files(folder, result)
}

pub(super) fn open_folder(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    folder: String,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let wanted = folder.clone();
    let handle = wiring
        .backend
        .spawn(async move { files_arrived(&service, &id, wanted).await });
    cx.spawn(async move |cx| {
        let arrived = match handle.await {
            Ok(arrived) => arrived,
            Err(error) => Arrived::Files(folder, Err(error.to_string())),
        };
        let _ = view.update(cx, |view, cx| view.arrived(arrived, cx));
    })
    .detach();
}

/// What a batch of content changes amounts to, in words.
pub(super) fn content_notice(results: &[ContentResult], did: &str) -> Result<String, String> {
    let failed: Vec<String> = results
        .iter()
        .filter_map(|result| {
            result
                .outcome
                .as_ref()
                .err()
                .map(|error| format!("{}: {error}", result.file_name))
        })
        .collect();
    if !failed.is_empty() {
        return Err(failed.join("\n"));
    }
    let changed = results
        .iter()
        .filter(|result| !matches!(result.outcome, Ok(ContentEffect::Unchanged)))
        .count();
    Ok(if changed == 0 {
        "没有需要改动的文件".to_owned()
    } else {
        format!("已{did} {changed} 个文件")
    })
}

/// Lists the game's top level and opens the export dialog over it.
pub(super) fn open_export(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    window: &mut Window,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let wanted = id.clone();
    let handle = wiring.backend.spawn(async move {
        let record = service.instance(&wanted).await?;
        let entries = service.list_files(&wanted, "").await?;
        // What is inside each folder, so single files can be left out. A
        // folder that cannot be read is just offered whole.
        let mut below = std::collections::BTreeMap::new();
        for folder in entries.iter().filter(|entry| entry.is_dir) {
            if let Ok(inside) = service.list_files(&wanted, &folder.name).await {
                below.insert(folder.name.clone(), inside);
            }
        }
        Ok::<_, ServiceError>((record.name, entries, below))
    });
    let wiring = wiring.clone();
    let window_handle = window.window_handle();
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let _ = cx.update_window(window_handle, |_, window, cx| match result {
            Ok((name, entries, below)) => {
                let form = cx.new(|cx| {
                    let (job_wiring, job_view, job_id) = (wiring.clone(), view.clone(), id.clone());
                    ExportForm::new(
                        &name,
                        &entries,
                        &below,
                        Rc::new(move |spec, window, cx| {
                            instance_intent(
                                &job_wiring,
                                &job_id,
                                job_view.clone(),
                                InstanceIntent::ExportPackAs(spec),
                                window,
                                cx,
                            );
                        }),
                        window,
                        cx,
                    )
                });
                ExportForm::open(form, window, cx);
            }
            Err(detail) => {
                let _ = view.update(cx, |view, cx| {
                    view.toast(
                        Toast::error("没能读取游戏的文件，无法导出").technical(detail),
                        cx,
                    )
                });
            }
        });
    })
    .detach();
}

/// Opens the dialog that changes a game's version and loader. What the person
/// picks becomes a write on the instance, so the page shows it busy, and a
/// failure leaves the game on its old version.
pub(super) fn open_runtime_change(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some((game_version, loader, loader_version)) =
        view.read_with(cx, |view, _| view.runtime()).ok().flatten()
    else {
        return;
    };
    let wiring = wiring.clone();
    window.defer(cx, move |window, cx| {
        let handle = window.window_handle();
        let form = cx.new(|cx| {
            let form: WeakEntity<NewGameForm> = cx.weak_entity();
            let (on_form, on_snapshot) = (wiring.clone(), wiring.clone());
            let (form_id, form_view) = (id.clone(), view.clone());
            let (snapshot_id, snapshot_view) = (id.clone(), view.clone());
            NewGameForm::for_change(
                Rc::new(move |intent, window, cx| match intent {
                    NewGameIntent::Create(request) => {
                        // The change runs as a write on the game; the dialog is done.
                        let _ = form.update(cx, |form, cx| form.created(Ok(()), cx));
                        instance_intent(
                            &on_form,
                            &form_id,
                            form_view.clone(),
                            InstanceIntent::ChangeRuntime {
                                loader: request.loader,
                                game_version: request.game_version,
                                loader_version: request.loader_version,
                            },
                            window,
                            cx,
                        );
                    }
                    other => new_game_intent(&on_form, form.clone(), handle, other, cx),
                }),
                RuntimeChange {
                    game_version,
                    loader,
                    loader_version,
                    snapshot: Rc::new(move |window, cx| {
                        instance_intent(
                            &on_snapshot,
                            &snapshot_id,
                            snapshot_view.clone(),
                            InstanceIntent::CreateSnapshot,
                            window,
                            cx,
                        );
                    }),
                },
                window,
                cx,
            )
        });
        NewGameForm::open(form, window, cx);
    });
}

/// Saves a game's log with names and paths hidden, where the person chooses.
pub(super) fn export_log(wiring: &Wiring, id: String, crash: Option<String>, cx: &mut App) {
    let start = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| wiring.backend.service.layout().root().to_owned());
    let suggested = match &crash {
        Some(name) => name.clone(),
        None => "latest-log.txt".to_owned(),
    };
    let chosen = platform::pick_save_path(cx, &start, &suggested);
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Some(path) = chosen.await else {
            return;
        };
        let service = wiring.backend.service.clone();
        let target = path.clone();
        let Ok(result) = wiring
            .backend
            .spawn(async move { service.export_log(&id, crash.as_deref(), &target).await })
            .await
        else {
            return;
        };
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.toast(
                match result {
                    Ok(()) => {
                        Toast::success(format!("已保存到 {}（名字和路径已隐去）", path.display()))
                    }
                    Err(error) => Toast::error("没能导出日志").technical(error.to_string()),
                },
                cx,
            )
        });
    })
    .detach();
}

/// What the window does around the game, as the preferences say: hide while
/// it runs, come back to the front when it ends.
pub(super) fn around_the_game(
    wiring: &Wiring,
    window: gpui_kit::AnyWindowHandle,
    signal: &LaunchSignal,
    after: AfterLaunch,
    cx: &mut gpui_kit::AsyncApp,
) {
    let preferences = wiring.state.borrow().preferences.clone();
    match signal {
        LaunchSignal::Running if after == AfterLaunch::Hide => {
            cx.update(|cx| platform::hide_launcher(cx));
        }
        LaunchSignal::Exited { .. } | LaunchSignal::Failed(_) | LaunchSignal::Cancelled
            if preferences.foreground_on_exit() =>
        {
            let _ = cx.update_window(window, |_, window, cx| platform::bring_to_front(window, cx));
        }
        _ => {}
    }
}
