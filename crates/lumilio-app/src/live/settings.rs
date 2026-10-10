use super::jobs::{job, outcome, reload};
use super::{POLL, Reload, Wiring};
use gpui_kit::AppContext as _;
use gpui_kit::{App, Window};
use lumilio_core::{
    CancellationToken, Preferences, ServiceError, import_wallpaper, remove_wallpapers,
};
use lumilio_ui::live::settings_view;
use lumilio_ui::platform;
use lumilio_ui::theme::{Catalog, scan_dir};
use lumilio_ui::toast::Toast;
use lumilio_ui::tr;
use std::future::Future;
use std::time::Duration;

/// Applies what the saved preferences say about the look, the motion and
/// the language.
pub(super) fn apply_preferences(
    wiring: &Wiring,
    preferences: &Preferences,
    window: &mut Window,
    cx: &mut App,
) {
    platform::apply_appearance(preferences.appearance, window, cx);
    platform::apply_look(&preferences.look, window, cx);
    apply_wallpaper(wiring, preferences.look.wallpaper.clone(), cx);
    platform::apply_motion(preferences.motion, cx);
    lumilio_ui::i18n::apply_language(preferences.language, cx);
    // Plugin text is the plugin's own; tell the host which tag to read it in
    // without blocking the UI thread.
    let service = wiring.backend.service.clone();
    let tag = lumilio_ui::i18n::locale().tag().to_owned();
    drop(
        wiring
            .backend
            .spawn(async move { service.set_plugin_locale(&tag).await }),
    );
}

/// Shows the chosen wallpaper once its file is found, off the UI thread. No
/// choice puts the world back and clears the stored pictures; a choice whose
/// file is gone shows the world and says so on the Appearance tab.
fn apply_wallpaper(wiring: &Wiring, chosen: Option<String>, cx: &mut App) {
    let dir = wiring.backend.service.layout().themes();
    let check = wiring.backend.spawn(async move {
        let Some(name) = chosen else {
            let cleared = dir.clone();
            let _ = tokio::task::spawn_blocking(move || remove_wallpapers(&cleared, None)).await;
            return (None, false);
        };
        // A hand-edited name must stay inside the folder.
        let plain = std::path::Path::new(&name).file_name() == Some(name.as_ref());
        let path = dir.join(&name);
        if plain
            && tokio::fs::metadata(&path)
                .await
                .is_ok_and(|meta| meta.is_file())
        {
            (Some(path), false)
        } else {
            (None, true)
        }
    });
    cx.spawn(async move |cx| {
        if let Ok((path, missing)) = check.await {
            cx.update(|cx| platform::apply_wallpaper(path, missing, cx));
        }
    })
    .detach();
}

/// Copies the chosen picture into the launcher's folder off the UI thread,
/// then saves it as the wallpaper through the ordinary preferences path.
pub(super) fn set_wallpaper(
    wiring: &Wiring,
    source: std::path::PathBuf,
    window: &Window,
    cx: &mut App,
) {
    let dir = wiring.backend.service.layout().themes();
    let import = wiring.backend.spawn(async move {
        tokio::task::spawn_blocking(move || import_wallpaper(&source, &dir)).await
    });
    let wiring = wiring.clone();
    let window = window.window_handle();
    cx.spawn(async move |cx| {
        let failure = match import.await {
            Ok(Ok(Ok(name))) => {
                let mut next = wiring.state.borrow().preferences.clone();
                next.look.wallpaper = Some(name);
                let _ = cx.update_window(window, |_, window, cx| {
                    super::dispatch::on_live_intent(
                        &wiring,
                        lumilio_ui::live::LiveIntent::SetPreferences(next),
                        window,
                        cx,
                    );
                });
                return;
            }
            Ok(Ok(Err(error))) => error.to_string(),
            Ok(Err(error)) => error.to_string(),
            Err(error) => error.to_string(),
        };
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.toast(
                Toast::error(tr!("settings-wallpaper-failed")).technical(failure),
                cx,
            )
        });
    })
    .detach();
}

pub(super) fn edit_plugin_setting(
    wiring: &Wiring,
    id: String,
    key: String,
    window: &Window,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let lookup_id = id.clone();
    let lookup_key = key.clone();
    let lookup = wiring.backend.spawn(async move {
        let info = service
            .plugins()
            .await
            .into_iter()
            .find(|info| info.manifest.id == lookup_id)?;
        let field = info
            .manifest
            .settings
            .iter()
            .find(|field| field.key == lookup_key)?
            .clone();
        let value = info
            .state
            .values
            .get(&lookup_key)
            .filter(|value| field.kind.accepts(value))
            .cloned()
            .unwrap_or_else(|| field.kind.default_value());
        Some((field, value))
    });
    let wiring = wiring.clone();
    let window = window.window_handle();
    cx.spawn(async move |cx| {
        let Ok(Some((field, value))) = lookup.await else {
            return;
        };
        let _ = cx.update_window(window, |_, window, cx| {
            let save_wiring = wiring.clone();
            let saved_wiring = wiring.clone();
            let save = std::rc::Rc::new(move |value| {
                let service = save_wiring.backend.service.clone();
                let (id, key) = (id.clone(), key.clone());
                let task = save_wiring
                    .backend
                    .spawn(async move { service.set_plugin_value(&id, &key, value).await });
                let pending: std::pin::Pin<Box<dyn Future<Output = Result<(), String>>>> =
                    Box::pin(async move {
                        match task.await {
                            Ok(Ok(())) => Ok(()),
                            _ => Err(tr!("plugin-setting-save-failed").to_owned()),
                        }
                    });
                pending
            });
            lumilio_ui::plugin_setting_dialog::PluginSettingDialog::open(
                field,
                value,
                save,
                std::rc::Rc::new(move |cx| load_settings(&saved_wiring, cx)),
                window,
                cx,
            );
        });
    })
    .detach();
}

/// Reads the local theme files off the UI thread and applies the look again
/// once they are in, so a theme chosen earlier shows as soon as it is found.
pub(super) fn reload_themes(wiring: &Wiring, window: gpui_kit::AnyWindowHandle, cx: &mut App) {
    let themes = wiring.backend.service.layout().themes();
    let scan = wiring
        .backend
        .spawn(async move { tokio::task::spawn_blocking(move || scan_dir(&themes)).await });
    cx.spawn(async move |cx| {
        if let Ok(Ok((entries, rejected))) = scan.await {
            let _ = cx.update_window(window, |_, window, cx| {
                platform::apply_catalog(Catalog::with_local(entries, rejected), window, cx);
            });
        }
    })
    .detach();
}

/// Reads the saved preferences once at start-up and applies them.
pub(super) fn apply_saved_preferences(
    wiring: &Wiring,
    window: gpui_kit::AnyWindowHandle,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.settings().await.preferences });
    reload_themes(wiring, window, cx);
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Ok(preferences) = handle.await else {
            return;
        };
        wiring.state.borrow_mut().preferences = preferences.clone();
        let _ = cx.update_window(window, |_, window, cx| {
            apply_preferences(&wiring, &preferences, window, cx)
        });
        // The Library comes back ordered and filtered as it was left, and
        // Discover keeps its advanced exclusions.
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.apply_discover_preferences(preferences.discover.clone(), cx);
            use lumilio_ui::kit::ViewIntent;
            use lumilio_ui::pages::live::{LIBRARY_LOADER, LIBRARY_SORT};
            shell.apply_view_intent(
                ViewIntent::Choose(LIBRARY_SORT, usize::from(preferences.library_sort)),
                cx,
            );
            shell.apply_view_intent(
                ViewIntent::Choose(LIBRARY_LOADER, usize::from(preferences.library_loader)),
                cx,
            );
        });
    })
    .detach();
}

/// Saves one setting. A failure is told with its raw cause; success says
/// `notice` (when there is one), applies `after` and shows the new values.
pub(super) fn change_setting(
    wiring: &Wiring,
    window: &Window,
    work: impl Future<Output = Result<(), ServiceError>> + Send + 'static,
    failed: &'static str,
    notice: Option<&'static str>,
    after: impl FnOnce(&mut Window, &mut App) + 'static,
    cx: &mut App,
) {
    let handle = wiring.backend.spawn(work);
    let wiring = wiring.clone();
    let window = window.window_handle();
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result,
            Err(error) => Err(ServiceError::Remote(error.to_string())),
        };
        match result {
            Ok(()) => {
                if let Some(notice) = notice {
                    let _ = wiring
                        .shell
                        .update(cx, |shell, cx| shell.toast(Toast::success(notice), cx));
                }
                let _ = cx.update_window(window, |_, window, cx| after(window, cx));
            }
            Err(error) => {
                let _ = wiring.shell.update(cx, |shell, cx| {
                    shell.toast(Toast::error(failed).technical(error.to_string()), cx)
                });
            }
        }
        cx.update(|cx| load_settings(&wiring, cx));
    })
    .detach();
}

/// Reads the settings, the Java list and then the disk use for the Settings page.
pub(super) fn load_settings(wiring: &Wiring, cx: &mut App) {
    let service = wiring.backend.service.clone();
    let handle = wiring.backend.spawn(async move {
        let settings = service.settings().await;
        let plugins = service.plugins().await;
        let java = service.java_installations().await;
        let memory = lumilio_core::total_memory_mb();
        (
            settings,
            plugins,
            java,
            memory,
            service.layout().root().to_owned(),
        )
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Ok((settings, plugins, java, memory, root)) = handle.await else {
            return;
        };
        wiring.state.borrow_mut().preferences = settings.preferences.clone();
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.plugins_changed(&plugins, cx);
            shell.update_live(
                |model| {
                    let storage = model.settings.as_ref().and_then(|view| view.storage);
                    model.settings = Some(settings_view(&settings, &java, storage, &root, memory));
                    if let Some(view) = &mut model.settings {
                        view.plugins = plugins;
                    }
                },
                cx,
            );
        });
        let service = wiring.backend.service.clone();
        let Ok(usage) = wiring
            .backend
            .spawn(async move { service.storage_usage().await })
            .await
        else {
            return;
        };
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.update_live(
                |model| {
                    if let Some(view) = &mut model.settings {
                        view.storage = Some(usage);
                    }
                },
                cx,
            );
        });
    })
    .detach();
}

pub(super) fn add_java(wiring: &Wiring, cx: &mut App) {
    let chosen = platform::pick_path(cx, true, true, tr!("settings-java-pick"));
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Some(path) = chosen.await else {
            return;
        };
        let service = wiring.backend.service.clone();
        let Ok(result) = wiring
            .backend
            .spawn(async move { service.add_java(&path).await })
            .await
        else {
            return;
        };
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.toast(
                match result {
                    Ok(runtime) => {
                        Toast::success(tr!("settings-java-added", version = runtime.version()))
                    }
                    Err(ServiceError::NoJavaAt(_)) => Toast::error(tr!("settings-java-not-found")),
                    Err(error) => {
                        Toast::error(tr!("settings-java-add-failed")).technical(error.to_string())
                    }
                },
                cx,
            )
        });
        cx.update(|cx| load_settings(&wiring, cx));
    })
    .detach();
}

/// Measures what no game uses; if there is any, asks before removing it.
pub(super) fn check_reclaimable(wiring: &Wiring, window: &mut Window, cx: &mut App) {
    let service = wiring.backend.service.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.reclaimable().await });
    let wiring = wiring.clone();
    let window = window.window_handle();
    cx.spawn(async move |cx| {
        let Ok(result) = handle.await else {
            return;
        };
        match result {
            Err(error) => {
                let _ = wiring.shell.update(cx, |shell, cx| {
                    shell.toast(
                        Toast::error(tr!("settings-reclaim-failed")).technical(error.to_string()),
                        cx,
                    )
                });
            }
            Ok(found) if found.unused.is_empty() => {
                let _ = wiring.shell.update(cx, |shell, cx| {
                    shell.toast(Toast::info(tr!("settings-reclaim-none")), cx)
                });
            }
            Ok(found) => {
                let size = lumilio_ui::pages::settings::bytes_text(found.bytes());
                let kept = found.kept.clone();
                let _ = cx.update_window(window, |_, window, cx| {
                    lumilio_ui::collections::confirm_reclaim(
                        &size,
                        &kept,
                        {
                            let wiring = wiring.clone();
                            move |_, cx| reclaim(&wiring, cx)
                        },
                        window,
                        cx,
                    );
                });
            }
        }
    })
    .detach();
}

/// Removes the shared game files no game uses.
pub(super) fn reclaim(wiring: &Wiring, cx: &mut App) {
    let service = wiring.backend.service.clone();
    job(
        wiring,
        cx,
        false,
        Reload::Nothing,
        None,
        async move { service.reclaim().await },
        |shell, result, cx| {
            shell.toast(
                match result {
                    Ok(freed) => Toast::success(tr!(
                        "settings-cleaned",
                        size = lumilio_ui::pages::settings::bytes_text(freed)
                    )),
                    Err(error) => Toast::error(tr!("settings-clean-files-failed"))
                        .technical(error.to_string()),
                },
                cx,
            );
        },
    );
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(Duration::from_millis(800))
            .await;
        cx.update(|cx| load_settings(&wiring, cx));
    })
    .detach();
}

pub(super) fn clear_cache(wiring: &Wiring, cx: &mut App) {
    let service = wiring.backend.service.clone();
    job(
        wiring,
        cx,
        false,
        Reload::Nothing,
        None,
        async move { service.clear_cache().await },
        |shell, result, cx| {
            shell.toast(
                match result {
                    Ok(freed) => Toast::success(tr!(
                        "settings-cleaned",
                        size = lumilio_ui::pages::settings::bytes_text(freed)
                    )),
                    Err(error) => Toast::error(tr!("settings-clear-cache-failed"))
                        .technical(error.to_string()),
                },
                cx,
            );
        },
    );
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        // The sizes change once the deletion finishes.
        cx.background_executor()
            .timer(Duration::from_millis(800))
            .await;
        cx.update(|cx| load_settings(&wiring, cx));
    })
    .detach();
}

pub(super) fn clear_map_cache(wiring: &Wiring, cx: &mut App) {
    let service = wiring.backend.service.clone();
    job(
        wiring,
        cx,
        false,
        Reload::Nothing,
        None,
        async move { service.clear_map_cache().await },
        |shell, result, cx| {
            shell.toast(
                match result {
                    Ok(()) => Toast::success(tr!("map-cache-cleared")),
                    Err(error) => Toast::error(tr!("settings-clear-cache-failed"))
                        .technical(error.to_string()),
                },
                cx,
            );
        },
    );
}

pub(super) fn export_diagnostics(wiring: &Wiring, cx: &mut App) {
    let start = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| wiring.backend.service.layout().root().to_owned());
    let chosen = platform::pick_save_path(cx, &start, "lumilio-diagnostics.zip");
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Some(path) = chosen.await else {
            return;
        };
        let service = wiring.backend.service.clone();
        let target = path.clone();
        let Ok(result) = wiring
            .backend
            .spawn(async move { service.export_diagnostics(&target).await })
            .await
        else {
            return;
        };
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.toast(
                match result {
                    Ok(()) => Toast::success(tr!(
                        "settings-diagnostics-saved",
                        path = path.display().to_string()
                    )),
                    Err(error) => Toast::error(tr!("settings-diagnostics-failed"))
                        .technical(error.to_string()),
                },
                cx,
            )
        });
    })
    .detach();
}

/// Downloads Java into the launcher, showing progress in Activity, and
/// refreshes the Settings Java list when done.
pub(super) fn install_java(wiring: &Wiring, major: Option<u32>, cx: &mut App) {
    let service = wiring.backend.service.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.install_java(major, CancellationToken::new()).await });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.toast(Toast::info(tr!("settings-java-install-started")), cx)
        });
        while !handle.is_finished() {
            cx.background_executor().timer(POLL).await;
            cx.update(|cx| reload(&wiring, Reload::Lists, cx));
        }
        let Ok(result) = handle.await else {
            return;
        };
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.toast(
                outcome(
                    result,
                    |java| tr!("settings-java-installed", version = java.major()),
                    tr!("settings-java-install-failed").to_owned(),
                ),
                cx,
            )
        });
        cx.update(|cx| {
            reload(&wiring, Reload::All, cx);
            load_settings(&wiring, cx);
        });
    })
    .detach();
}
