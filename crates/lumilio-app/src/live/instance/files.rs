use std::rc::Rc;

use gpui_kit::{App, AppContext as _, WeakEntity, Window};
use lumilio_core::{AfterLaunch, LaunchSignal, ServiceError};
use lumilio_ui::export_form::ExportForm;
use lumilio_ui::features::new_game::{NewGameForm, NewGameIntent, RuntimeChange, answer_versions};
use lumilio_ui::instance_detail::{Arrived, InstanceDetailView, InstanceIntent};
use lumilio_ui::toast::Toast;
use lumilio_ui::{platform, tr};

use super::super::Wiring;
use super::{files_arrived, instance_intent};

/// Makes one screenshot's thumbnail off the interface thread.
pub(super) fn make_thumbnail(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    file: String,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let asked = file.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.screenshot_thumbnail(&id, &asked).await });
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
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    file: String,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.screenshot_bytes(&id, &file).await });
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
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    address: String,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let asked = address.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.server_status(&id, &asked).await });
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

pub(in super::super) fn open_crash(
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

pub(in super::super) fn open_folder(
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

/// Lists the game's top level and opens the export dialog over it.
pub(in super::super) fn open_export(
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
pub(in super::super) fn open_runtime_change(
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
                    other => answer_versions(&on_form.backend.worker(), form.clone(), other, cx),
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
pub(in super::super) fn export_log(
    wiring: &Wiring,
    id: String,
    crash: Option<String>,
    cx: &mut App,
) {
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
                    Ok(()) => Toast::success(tr!("logs-saved", path = path.display().to_string())),
                    Err(error) => {
                        Toast::error(tr!("logs-export-failed")).technical(error.to_string())
                    }
                },
                cx,
            )
        });
    })
    .detach();
}

/// What the window does around the game, as the preferences say: hide while
/// it runs, come back to the front when it ends.
pub(in super::super) fn around_the_game(
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
