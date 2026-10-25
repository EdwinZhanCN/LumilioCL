//! The instance page's plugin tabs: asks the service which tabs show and what
//! they contain, and performs the effects a plugin's action asks for.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyWindowHandle, App, AppContext as _, Entity, Render, Task, TitlebarOptions, WeakEntity,
    Window, WindowBounds, WindowOptions, px, size,
};
use lumilio_core::PluginEffect;
use lumilio_plugin_api::ActionId;

use super::game_actions::GameWork;
use crate::instance_detail::InstanceDetailView;
use crate::schematic_studio::{StudioAction, StudioView, StudioWindow};
use crate::toast::Toast;
use crate::world_explorer::{Command, Connection, Event, MapHandoff, MapWindow};
use crate::{platform, tr};

/// Opens a window of the launcher's, under the framework's root (dialogs and
/// toasts need it), and hands back the window and what it shows.
fn open_window<V: Render>(
    options: WindowOptions,
    cx: &mut App,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> gpui::Result<(AnyWindowHandle, Entity<V>)> {
    let mut built = None;
    let window = cx.open_window(options, |window, cx| {
        let view = build(window, cx);
        built = Some(view.clone());
        cx.new(|cx| gpui_component::Root::new(view, window, cx))
    })?;
    Ok((
        window.into(),
        built.expect("open_window ran its build closure"),
    ))
}

/// Where a plugin action's toasts go: the game page, or a studio window.
#[derive(Clone)]
enum ToastSink {
    Page(WeakEntity<InstanceDetailView>),
    Studio(WeakEntity<StudioView>),
}

impl ToastSink {
    fn toast(&self, toast: Toast, cx: &mut gpui::AsyncApp) {
        match self {
            Self::Page(view) => {
                let _ = view.update(cx, |view, cx| view.toast(toast, cx));
            }
            Self::Studio(studio) => {
                let _ = studio.update(cx, |studio, cx| studio.toast(toast, cx));
            }
        }
    }
}

/// One open studio window (plan schematic-studio, D22): one per game and
/// folder. The studio entity dies with its window, so a dead `studio` means
/// the window is gone.
pub(super) struct StudioWindowEntry {
    pub(super) id: String,
    folder: String,
    window: AnyWindowHandle,
    pub(super) studio: WeakEntity<StudioView>,
}

/// Opens the studio window for `plugin`'s `folder` on this game, or brings
/// the open one forward, and chooses `file` in it when there is one. The
/// window's connection, viewport loads and plugin actions are made here and
/// belong to that window alone.
pub(super) fn open_studio_window(
    work: &GameWork,
    id: String,
    plugin: String,
    folder: String,
    file: Option<String>,
    cx: &mut App,
) {
    let existing = {
        let mut studios = work.studios.borrow_mut();
        studios.retain(|entry| entry.studio.upgrade().is_some());
        studios
            .iter()
            .find(|entry| entry.id == id && entry.folder == folder)
            .map(|entry| (entry.window, entry.studio.clone()))
    };
    if let Some((window, studio)) = existing {
        let _ = window.update(cx, |_, window, _| window.activate_window());
        if let Some(file) = file {
            let _ = studio.update(cx, |studio, cx| studio.select_file(file, cx));
        }
        return;
    }
    let connection = studio_connection(work, id.clone(), plugin.clone(), folder.clone());
    // The studio exists only once the window is built; the closures it is
    // given reach it through this slot.
    let slot: Rc<RefCell<Option<WeakEntity<StudioView>>>> = Rc::default();
    let load_model = {
        let (work, id, plugin, slot) = (work.clone(), id.clone(), plugin.clone(), slot.clone());
        Rc::new(
            move |file: String, request: u64, _: &mut gpui::Window, cx: &mut App| {
                let Some(studio) = slot.borrow().clone() else {
                    return;
                };
                let service = work.service().clone();
                let (id, plugin) = (id.clone(), plugin.clone());
                let handle =
                    work.spawn(async move { service.plugin_model(&id, &plugin, &file).await });
                cx.spawn(async move |cx| {
                    let result = match handle.await {
                        Ok(result) => result.map_err(|error| error.to_string()),
                        Err(error) => Err(error.to_string()),
                    };
                    let _ = studio.update(cx, |studio, cx| {
                        studio.model_arrived(request, result, cx);
                    });
                })
                .detach();
            },
        )
    };
    let act = {
        let (work, id, plugin, slot) = (work.clone(), id.clone(), plugin.clone(), slot.clone());
        Rc::new(
            move |action: StudioAction, _: &mut gpui::Window, cx: &mut App| {
                let Some(studio) = slot.borrow().clone() else {
                    return;
                };
                let action = match action {
                    StudioAction::Reveal(file) => format!("reveal:{file}"),
                    StudioAction::ExportCsv(file) => format!("export:{file}"),
                };
                run_plugin_action(
                    &work,
                    id.clone(),
                    plugin.clone(),
                    ActionId::new(action),
                    ToastSink::Studio(studio),
                    cx,
                )
                .detach();
            },
        )
    };
    // The review hooks size this window as they size the main one, so a
    // screenshot can show the studio at its smallest (plan schematic-studio
    // T19).
    let (width, height) = work.windows().studio_size.unwrap_or((1120., 760.));
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(width), px(height)), cx)),
        window_min_size: Some(size(px(720.), px(480.))),
        titlebar: Some(TitlebarOptions {
            title: Some(tr!("studio-window-title").into()),
            ..crate::window_titlebar()
        }),
        app_id: work.windows().app_id.clone(),
        ..WindowOptions::default()
    };
    let (for_plugin, for_folder) = (plugin.clone(), folder.clone());
    let opened = open_window(options, cx, move |window, cx| {
        crate::follow_system_appearance(window, cx);
        cx.new(|cx| {
            StudioWindow::new(
                for_plugin, for_folder, file, load_model, act, connection, cx,
            )
        })
    });
    match opened {
        Ok((window, root)) => {
            let studio = root.read(cx).studio().downgrade();
            *slot.borrow_mut() = Some(studio.clone());
            work.studios.borrow_mut().push(StudioWindowEntry {
                id,
                folder,
                window,
                studio,
            });
        }
        Err(reason) => log::error!("could not open the studio window: {reason}"),
    }
}

pub(super) fn plugin_map(
    work: &GameWork,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    request: u64,
    cx: &mut App,
) {
    let connection = map_connection(work, id);
    let _ = view.update(cx, |view, cx| {
        view.plugin_map_arrived(request, connection, cx)
    });
}

/// Connects a schematic studio to its plugin's folder: listing and reading
/// go through the plugin's grants; writes, moves to the backups, history and
/// restore through the service's schematic operations (plan schematic-studio,
/// T9/T12–T16).
pub(super) fn plugin_studio(
    work: &GameWork,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    plugin: String,
    folder: String,
    request: u64,
    cx: &mut App,
) {
    let connection = studio_connection(work, id, plugin, folder);
    let _ = view.update(cx, |view, cx| {
        view.plugin_studio_arrived(request, connection, cx)
    });
}

/// A new connection between one studio (in the page or in its window) and
/// the service: commands go in, answers come back as events.
fn studio_connection(
    work: &GameWork,
    id: String,
    plugin: String,
    folder: String,
) -> crate::schematic_studio::Connection {
    use crate::schematic_studio::{
        Command as StudioCommand, Connection as StudioConnection, Event as StudioEvent,
        WriteFailure,
    };
    fn failure(error: lumilio_core::ServiceError) -> WriteFailure {
        match &error {
            lumilio_core::ServiceError::InstanceBusy(_) => WriteFailure::Busy,
            lumilio_core::ServiceError::Plugin(lumilio_plugin_api::PluginError::Unavailable(
                reason,
            )) if reason == lumilio_core::WRITE_CONFLICT => WriteFailure::Conflict,
            _ => WriteFailure::Other(error.to_string()),
        }
    }
    let (send, commands) = async_channel::bounded(256);
    let (events, receive) = async_channel::bounded(256);
    let service = work.service().clone();
    work.spawn(async move {
        let mut tasks = tokio::task::JoinSet::new();
        while let Ok(command) = commands.recv().await {
            let (service, id, plugin, folder, events) = (
                service.clone(),
                id.clone(),
                plugin.clone(),
                folder.clone(),
                events.clone(),
            );
            tasks.spawn(async move {
                let text = |error: lumilio_core::ServiceError| error.to_string();
                let event = match command {
                    StudioCommand::List => StudioEvent::Listed(
                        service
                            .schematic_files(&id, &plugin, &folder)
                            .await
                            .map_err(text),
                    ),
                    StudioCommand::Read { file } => {
                        let result = service
                            .schematic_read(&id, &plugin, &file)
                            .await
                            .map_err(text);
                        StudioEvent::Read { file, result }
                    }
                    StudioCommand::History => {
                        StudioEvent::History(service.schematic_history(&id).await.map_err(text))
                    }
                    StudioCommand::Backups => {
                        StudioEvent::Backups(service.schematic_backups(&id).await.map_err(text))
                    }
                    StudioCommand::RestoreBackup { backup } => StudioEvent::Restored(
                        service
                            .schematic_restore_backup(&id, &plugin, backup)
                            .await
                            .map_err(text),
                    ),
                    StudioCommand::CanWrite => {
                        StudioEvent::CanWrite(service.schematic_can_write(&id))
                    }
                    StudioCommand::Restore { index } => StudioEvent::Restored(
                        service
                            .schematic_restore(&id, &plugin, index)
                            .await
                            .map_err(text),
                    ),
                    StudioCommand::Apply {
                        file,
                        bytes,
                        expected,
                        operation,
                    } => {
                        let result = service
                            .schematic_apply(&id, &plugin, &file, bytes, expected, operation)
                            .await
                            .map_err(failure);
                        StudioEvent::Applied { file, result }
                    }
                    StudioCommand::Remove { files, operation } => StudioEvent::Removed(
                        service
                            .schematic_remove(&id, &plugin, files, operation)
                            .await
                            .map_err(failure),
                    ),
                    StudioCommand::Versions => {
                        StudioEvent::Versions(service.schematic_versions().await.map_err(text))
                    }
                    StudioCommand::Schemes => {
                        StudioEvent::Schemes(service.schematic_schemes().await.map_err(text))
                    }
                    StudioCommand::SaveSchemes(schemes) => StudioEvent::Schemes(
                        service.schematic_schemes_save(schemes).await.map_err(text),
                    ),
                    StudioCommand::Game => StudioEvent::Game(
                        service
                            .instance(&id)
                            .await
                            .map(|record| record.game_version)
                            .map_err(text),
                    ),
                    StudioCommand::Catalogs {
                        target,
                        source_data_version,
                    } => {
                        let result = service
                            .schematic_catalogs(&target, source_data_version)
                            .await
                            .map_err(text);
                        StudioEvent::Catalogs { target, result }
                    }
                    // An added pack is switched on for the folder it was
                    // added from (plan schematic-studio, T21).
                    StudioCommand::AddPack { path } => StudioEvent::PacksChanged(
                        service
                            .schematic_pack_add(&id, &folder, path)
                            .await
                            .map_err(text),
                    ),
                    StudioCommand::RemovePack { path } => StudioEvent::PacksChanged(
                        service.schematic_pack_remove(&id, path).await.map_err(text),
                    ),
                    StudioCommand::SetSource { path, on } => {
                        let result = service
                            .schematic_source_set(&id, &folder, path.clone(), on)
                            .await
                            .map_err(text);
                        StudioEvent::SourceSwitched { path, on, result }
                    }
                    StudioCommand::MoveSource { path, past } => StudioEvent::SourceMoved(
                        service
                            .schematic_source_move(&id, path, past)
                            .await
                            .map_err(text),
                    ),
                    StudioCommand::Names => {
                        StudioEvent::Names(service.schematic_names(&id).await.map_err(text))
                    }
                    // Drawn once per content and look of the sources (plan
                    // schematic-studio T28, T21), off the async threads.
                    StudioCommand::Thumbnail { file, key } => {
                        let result = service
                            .schematic_thumbnail(
                                &id,
                                &plugin,
                                &file,
                                &key,
                                crate::schematic_studio::render_thumbnail,
                            )
                            .await
                            .map_err(text);
                        StudioEvent::Thumbnail { file, result }
                    }
                };
                let _ = events.send(event).await;
            });
            while tasks.try_join_next().is_some() {}
        }
        tasks.abort_all();
    });
    StudioConnection { send, receive }
}

/// Opens the map in a window of its own. Its connection is made here and
/// belongs to that window alone, so closing the window ends only that
/// connection.
pub(super) fn pop_out_map(work: &GameWork, id: String, handoff: MapHandoff, cx: &mut App) {
    let connection = map_connection(work, id);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(1080.), px(720.)), cx)),
        window_min_size: Some(size(px(480.), px(320.))),
        titlebar: Some(TitlebarOptions {
            title: Some(tr!("map-window-title").into()),
            ..crate::window_titlebar()
        }),
        app_id: work.windows().app_id.clone(),
        ..WindowOptions::default()
    };
    let opened = open_window(options, cx, move |window, cx| {
        crate::follow_system_appearance(window, cx);
        cx.new(|cx| MapWindow::new(handoff, connection, cx))
    });
    if let Err(reason) = opened {
        log::error!("could not open the map window: {reason}");
    }
}

/// A new connection between one map view and the service: commands go in,
/// answers come back as events.
fn map_connection(work: &GameWork, id: String) -> Connection {
    let (send, commands) = async_channel::bounded(128);
    let (events, receive) = async_channel::bounded(128);
    let service = work.service().clone();
    work.spawn(async move {
        let mut tasks = tokio::task::JoinSet::new();
        while let Ok(command) = commands.recv().await {
            let (service, id, events) = (service.clone(), id.clone(), events.clone());
            tasks.spawn(async move {
                let event = match command {
                    Command::SaveSeed { seed, version } => {
                        let result = async {
                            service.save_map_seed(&id, seed, &version).await.map_err(|error|error.to_string())?;
                            let contexts=service.map_contexts(&id).await.map_err(|error|error.to_string())?;
                            let context=contexts.iter().find(|world|matches!(&world.context.world,lumilio_plugin_api::map::WorldId::Seed { seed: saved,version: name } if *saved==seed && *name==version)).ok_or_else(||"saved seed unavailable".to_owned())?.context.clone();
                            Ok((contexts,context))
                        }.await;
                        Event::Seed(result)
                    },
                    Command::Contexts => {
                        let providers = service.map_providers().await;
                        Event::Contexts(
                            service
                                .map_contexts(&id)
                                .await
                                .map(|contexts| (contexts, providers))
                                .map_err(|error| error.to_string()),
                        )
                    }
                    Command::Annotations { context } => Event::Annotations {
                        result: service
                            .map_annotations(&id, &context.world, &context.dimension)
                            .await
                            .map_err(|error| error.to_string()),
                        context,
                    },
                    Command::PutAnnotation { context, annotation } => {
                        let result = async {
                            service
                                .put_map_annotation(&id, *annotation)
                                .await
                                .map_err(|error| error.to_string())?;
                            service
                                .map_annotations(&id, &context.world, &context.dimension)
                                .await
                                .map_err(|error| error.to_string())
                        }
                        .await;
                        Event::Annotations { context, result }
                    }
                    Command::RemoveAnnotation { context, id: annotation_id } => {
                        let result = async {
                            service
                                .remove_map_annotation(&id, annotation_id)
                                .await
                                .map_err(|error| error.to_string())?;
                            service
                                .map_annotations(&id, &context.world, &context.dimension)
                                .await
                                .map_err(|error| error.to_string())
                        }
                        .await;
                        Event::Annotations { context, result }
                    }
                    Command::CanEdit => Event::CanEdit(service.map_can_edit(&id)),
                    Command::Apply { plugin, edit } => Event::Applied(
                        service
                            .map_apply(&id, &plugin, *edit)
                            .await
                            .map_err(|failure| match failure {
                                lumilio_core::MapFailure::Failed(reason) => reason,
                                _ => "map-edit-failed".to_owned(),
                            }),
                    ),
                    Command::LinkXaero { folder, dir } => {
                        let result = async {
                            service
                                .link_map_xaero(&id, &folder, &dir)
                                .await
                                .map_err(|error| error.to_string())?;
                            service
                                .map_contexts(&id)
                                .await
                                .map_err(|error| error.to_string())
                        }
                        .await;
                        Event::Linked(result)
                    }
                    Command::Overlays { context } => Event::Overlays {
                        layers: service.map_overlays(&context).await,
                        context,
                    },
                    Command::Objects {
                        generation,
                        plugin,
                        request,
                        key,
                        cancel,
                    } => Event::Objects {
                        generation,
                        key,
                        result: service.map_objects(&id, &plugin, *request, cancel).await,
                    },
                    Command::Tile {
                        generation,
                        request,
                        cancel,
                    } => {
                        let key = request.key.clone();
                        Event::Tile {
                            generation,
                            key,
                            result: service.map_tile(&id, *request, cancel).await,
                        }
                    }
                };
                let _ = events.send(event).await;
            });
            while tasks.try_join_next().is_some() {}
        }
        tasks.abort_all();
    });
    Connection { send, receive }
}

pub(super) fn plugin_tabs(
    work: &GameWork,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    cx: &mut App,
) {
    let service = work.service().clone();
    let handle = work.spawn(async move { service.plugin_tabs(&id).await });
    cx.spawn(async move |cx| {
        // A game that cannot be read has no plugin tabs; the page reports that itself.
        let tabs = crate::runtime::answer(handle, cx)
            .await
            .and_then(Result::ok)
            .unwrap_or_default();
        let _ = view.update(cx, |view, cx| view.plugin_tabs_arrived(tabs, cx));
    })
    .detach();
}

pub(super) fn plugin_view(
    work: &GameWork,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    plugin: String,
    cx: &mut App,
) {
    let service = work.service().clone();
    let asked = plugin.clone();
    let handle = work.spawn(async move { service.plugin_view(&id, &asked).await });
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let _ = view.update(cx, |view, cx| view.plugin_view_arrived(plugin, result, cx));
    })
    .detach();
}

/// Reads assets off the UI thread and answers only the requesting inline view.
pub(super) fn plugin_model(
    work: &GameWork,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    plugin: String,
    file: String,
    request: u64,
    cx: &mut App,
) {
    let service = work.service().clone();
    let handle = work.spawn(async move { service.plugin_model(&id, &plugin, &file).await });
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let _ = view.update(cx, |view, cx| {
            view.plugin_model_arrived(request, result, cx)
        });
    })
    .detach();
}

/// Runs the action, performs the effects the host let through, then shows the
/// tab's new view.
pub(super) fn plugin_action(
    work: &GameWork,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    plugin: String,
    action: ActionId,
    cx: &mut App,
) {
    let done = run_plugin_action(
        work,
        id.clone(),
        plugin.clone(),
        action,
        ToastSink::Page(view.clone()),
        cx,
    );
    let work = work.clone();
    cx.spawn(async move |cx| {
        done.await;
        cx.update(|cx| plugin_view(&work, id, view, plugin, cx));
    })
    .detach();
}

/// Runs the action and performs the effects the host let through; what the
/// plugin says goes to `sink`. The caller decides whether a view follows.
fn run_plugin_action(
    work: &GameWork,
    id: String,
    plugin: String,
    action: ActionId,
    sink: ToastSink,
    cx: &mut App,
) -> Task<()> {
    let service = work.service().clone();
    let handle = work.spawn(async move { service.plugin_action(&id, &plugin, action).await });
    let work = work.clone();
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        match result {
            Ok(Some(effects)) => {
                for effect in effects {
                    perform(&work, effect, &sink, cx).await;
                }
            }
            Ok(None) => sink.toast(Toast::error(tr!("instance-plugin-unavailable")), cx),
            Err(detail) => sink.toast(
                Toast::error(tr!("instance-plugin-action-failed")).technical(detail),
                cx,
            ),
        }
    })
}

async fn perform(work: &GameWork, effect: PluginEffect, sink: &ToastSink, cx: &mut gpui::AsyncApp) {
    match effect {
        PluginEffect::Reveal(path) => {
            cx.update(|cx| platform::reveal(&path, cx));
        }
        PluginEffect::Toast(text) => sink.toast(Toast::success(text), cx),
        PluginEffect::SaveAs {
            suggested_name,
            bytes,
        } => {
            // Choosing the place is the person's consent to this one write.
            let start = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| work.service().layout().root().to_owned());
            let chosen = cx.update(|cx| platform::pick_save_path(cx, &start, &suggested_name));
            let Some(path) = chosen.await else {
                return;
            };
            let target = path.clone();
            let written = work
                .spawn(async move { tokio::fs::write(&target, bytes).await })
                .await;
            let toast = match written {
                Ok(Ok(())) => {
                    Toast::success(tr!("instance-saved-to", path = path.display().to_string()))
                }
                Ok(Err(error)) => {
                    Toast::error(tr!("instance-save-failed")).technical(error.to_string())
                }
                Err(error) => {
                    Toast::error(tr!("instance-save-failed")).technical(error.to_string())
                }
            };
            sink.toast(toast, cx);
        }
    }
}
