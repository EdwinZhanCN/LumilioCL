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
