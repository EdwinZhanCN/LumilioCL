//! A game's page beyond its sections: screenshots, server pings, crash
//! reports, folders, the export and change-version dialogs, and saving logs.

use std::rc::Rc;

use gpui::{App, AppContext as _, WeakEntity, Window};
use lumilio_core::ServiceError;

use super::answer::answer;
use super::game_actions::GameWork;
use super::load::files_arrived;
use crate::export_form::ExportForm;
use crate::features::new_game::{NewGameForm, NewGameIntent, RuntimeChange, answer_versions};
use crate::instance_detail::{Arrived, InstanceDetailView, InstanceIntent};
use crate::toast::{self, Toast};
use crate::{platform, tr};

/// Makes one screenshot's thumbnail off the interface thread.
pub(super) fn make_thumbnail(
    work: &GameWork,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    file: String,
    cx: &mut App,
) {
    let service = work.service().clone();
    let asked = file.clone();
    let handle = work.spawn(async move { service.screenshot_thumbnail(&id, &asked).await });
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let _ = view.update(cx, |view, cx| view.thumbnail_arrived(file, result, cx));
    })
    .detach();
}

/// Puts a screenshot on the clipboard as a picture.
pub(super) fn copy_screenshot(
    work: &GameWork,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    file: String,
    cx: &mut App,
) {
    let service = work.service().clone();
    let handle = work.spawn(async move { service.screenshot_bytes(&id, &file).await });
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let _ = view.update(cx, |view, cx| match result {
            Ok(bytes) => {
                platform::copy_image(bytes, cx);
                view.toast(Toast::success(tr!("instance-screenshot-copied")), cx);
            }
            Err(detail) => view.toast(
                Toast::error(tr!("instance-screenshot-copy-failed")).technical(detail),
                cx,
            ),
        });
    })
    .detach();
}

/// Asks one server how it is, off the interface thread. The instance's game
/// version decides the handshake (the legacy ping for a pre-1.7 version).
pub(super) fn ping_server(
    work: &GameWork,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    address: String,
    cx: &mut App,
) {
    let service = work.service().clone();
    let asked = address.clone();
    let handle = work.spawn(async move { service.server_status(&id, &asked).await });
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let _ = view.update(cx, |view, cx| {
            view.server_status_arrived(address, result, cx)
        });
    })
    .detach();
}

pub(super) fn open_crash(
    work: &GameWork,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    file: String,
    cx: &mut App,
) {
    let service = work.service().clone();
    let name = file.clone();
    let handle = work.spawn(async move { service.crash_report(&id, &name).await });
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let _ = view.update(cx, |view, cx| view.crash_arrived(file, result, cx));
    })
    .detach();
}

pub(super) fn open_folder(
    work: &GameWork,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    folder: String,
    cx: &mut App,
) {
    let service = work.service().clone();
    let wanted = folder.clone();
    let handle = work.spawn(async move { files_arrived(&service, &id, wanted).await });
    cx.spawn(async move |cx| {
        let arrived = match handle.await {
            Ok(arrived) => arrived,
            Err(error) => Arrived::Files(folder, Err(error.to_string())),
        };
        let _ = view.update(cx, |view, cx| view.arrived(arrived, cx));
    })
    .detach();
}

/// Lists the game's top level and opens the export dialog over it.
pub(super) fn open_export(
    work: &GameWork,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    window: &mut Window,
    cx: &mut App,
) {
    let service = work.service().clone();
    let wanted = id.clone();
    let handle = work.spawn(async move {
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
    let work = work.clone();
    let window_handle = window.window_handle();
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let _ = cx.update_window(window_handle, |_, window, cx| match result {
            Ok((name, entries, below)) => {
                let form = cx.new(|cx| {
                    let (job_wiring, job_view, job_id) = (work.clone(), view.clone(), id.clone());
                    ExportForm::new(
                        &name,
                        &entries,
                        &below,
                        Rc::new(move |spec, window, cx| {
                            answer(
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
                        Toast::error(tr!("instance-export-unavailable")).technical(detail),
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
    work: &GameWork,
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
    let work = work.clone();
    window.defer(cx, move |window, cx| {
        let form = cx.new(|cx| {
            let form: WeakEntity<NewGameForm> = cx.weak_entity();
            let (on_form, on_snapshot) = (work.clone(), work.clone());
            let (form_id, form_view) = (id.clone(), view.clone());
            let (snapshot_id, snapshot_view) = (id.clone(), view.clone());
            NewGameForm::for_change(
                Rc::new(move |intent, window, cx| match intent {
                    NewGameIntent::Create(request) => {
                        // The change runs as a write on the game; the dialog is done.
                        let _ = form.update(cx, |form, cx| form.created(Ok(()), cx));
                        answer(
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
                    other => answer_versions(on_form.worker(), form.clone(), other, cx),
                }),
                RuntimeChange {
                    game_version,
                    loader,
                    loader_version,
                    snapshot: Rc::new(move |window, cx| {
                        answer(
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
pub(super) fn export_log(work: &GameWork, id: String, crash: Option<String>, cx: &mut App) {
    let start = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| work.service().layout().root().to_owned());
    let suggested = match &crash {
        Some(name) => name.clone(),
        None => "latest-log.txt".to_owned(),
    };
    let chosen = platform::pick_save_path(cx, &start, &suggested);
    let work = work.clone();
    cx.spawn(async move |cx| {
        let Some(path) = chosen.await else {
            return;
        };
        let service = work.service().clone();
        let target = path.clone();
        let Ok(result) = work
            .spawn(async move { service.export_log(&id, crash.as_deref(), &target).await })
            .await
        else {
            return;
        };
        let told = match result {
            Ok(()) => Toast::success(tr!("logs-saved", path = path.display().to_string())),
            Err(error) => Toast::error(tr!("logs-export-failed")).technical(error.to_string()),
        };
        cx.update(|cx| toast::post(told, cx));
    })
    .detach();
}
