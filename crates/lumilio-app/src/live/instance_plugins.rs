//! The instance page's plugin tabs: asks the service which tabs show and what
//! they contain, and performs the effects a plugin's action asks for.

use super::Wiring;
use gpui_kit::{App, WeakEntity};
use lumilio_core::PluginEffect;
use lumilio_plugin_api::ActionId;
use lumilio_ui::instance_detail::InstanceDetailView;
use lumilio_ui::platform;
use lumilio_ui::toast::Toast;
use lumilio_ui::tr;

pub(super) fn plugin_map(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    request: u64,
    cx: &mut App,
) {
    use lumilio_ui::world_explorer::{Command, Connection, Event};
    let (send, commands) = async_channel::bounded(128);
    let (events, receive) = async_channel::bounded(128);
    let service = wiring.backend.service.clone();
    wiring.backend.spawn(async move {
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
    let _ = view.update(cx, |view, cx| {
        view.plugin_map_arrived(request, Connection { send, receive }, cx)
    });
}

pub(super) fn plugin_tabs(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.plugin_tabs(&id).await });
    cx.spawn(async move |cx| {
        // A game that cannot be read has no plugin tabs; the page reports that itself.
        let tabs = handle.await.ok().and_then(Result::ok).unwrap_or_default();
        let _ = view.update(cx, |view, cx| view.plugin_tabs_arrived(tabs, cx));
    })
    .detach();
}

pub(super) fn plugin_view(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    plugin: String,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let asked = plugin.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.plugin_view(&id, &asked).await });
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
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    plugin: String,
    file: String,
    request: u64,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.plugin_model(&id, &plugin, &file).await });
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
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    plugin: String,
    action: ActionId,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let (asked, instance) = (plugin.clone(), id.clone());
    let handle = wiring
        .backend
        .spawn(async move { service.plugin_action(&instance, &asked, action).await });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        match result {
            Ok(Some(effects)) => {
                for effect in effects {
                    perform(&wiring, effect, view.clone(), cx).await;
                }
            }
            Ok(None) => {
                let _ = view.update(cx, |view, cx| {
                    view.toast(Toast::error(tr!("instance-plugin-unavailable")), cx);
                });
            }
            Err(detail) => {
                let _ = view.update(cx, |view, cx| {
                    view.toast(
                        Toast::error(tr!("instance-plugin-action-failed")).technical(detail),
                        cx,
                    );
                });
            }
        }
        cx.update(|cx| plugin_view(&wiring, id, view, plugin, cx));
    })
    .detach();
}

async fn perform(
    wiring: &Wiring,
    effect: PluginEffect,
    view: WeakEntity<InstanceDetailView>,
    cx: &mut gpui_kit::AsyncApp,
) {
    match effect {
        PluginEffect::Reveal(path) => {
            cx.update(|cx| platform::reveal(&path, cx));
        }
        PluginEffect::Toast(text) => {
            let _ = view.update(cx, |view, cx| view.toast(Toast::success(text), cx));
        }
        PluginEffect::SaveAs {
            suggested_name,
            bytes,
        } => {
            // Choosing the place is the person's consent to this one write.
            let start = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| wiring.backend.service.layout().root().to_owned());
            let chosen = cx.update(|cx| platform::pick_save_path(cx, &start, &suggested_name));
            let Some(path) = chosen.await else {
                return;
            };
            let target = path.clone();
            let written = wiring
                .backend
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
            let _ = view.update(cx, |view, cx| view.toast(toast, cx));
        }
    }
}
