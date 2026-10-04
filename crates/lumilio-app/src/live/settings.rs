use super::jobs::{job, outcome, reload};
use super::{POLL, Reload, Wiring};
use gpui_kit::AppContext as _;
use gpui_kit::{App, Window};
use lumilio_core::{CancellationToken, Preferences, ServiceError};
use lumilio_ui::live::settings_view;
use lumilio_ui::platform;
use lumilio_ui::toast::Toast;
use std::future::Future;
use std::time::Duration;

/// Applies what the saved preferences say about the look and the motion.
pub(super) fn apply_preferences(preferences: &Preferences, window: &mut Window, cx: &mut App) {
    platform::apply_appearance(preferences.appearance, window, cx);
    platform::apply_motion(preferences.motion, cx);
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
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Ok(preferences) = handle.await else {
            return;
        };
        wiring.state.borrow_mut().preferences = preferences.clone();
        let _ = cx.update_window(window, |_, window, cx| {
            apply_preferences(&preferences, window, cx)
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
        let java = service.java_installations().await;
        let memory = lumilio_core::total_memory_mb();
        (settings, java, memory, service.layout().root().to_owned())
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Ok((settings, java, memory, root)) = handle.await else {
            return;
        };
        wiring.state.borrow_mut().preferences = settings.preferences.clone();
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.update_live(
                |model| {
                    let storage = model.settings.as_ref().and_then(|view| view.storage);
                    model.settings = Some(settings_view(&settings, &java, storage, &root, memory));
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
    let chosen = platform::pick_path(cx, true, true, "选择 Java");
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
                    Ok(runtime) => Toast::success(format!("已添加 Java {}", runtime.version())),
                    Err(ServiceError::NoJavaAt(_)) => {
                        Toast::error("这里没有找到 Java，请选 java 程序或 JDK 文件夹")
                    }
                    Err(error) => Toast::error("没能添加 Java").technical(error.to_string()),
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
                        Toast::error("没能检查游戏文件").technical(error.to_string()),
                        cx,
                    )
                });
            }
            Ok(found) if found.unused.is_empty() => {
                let _ = wiring.shell.update(cx, |shell, cx| {
                    shell.toast(Toast::info("没有多余的游戏文件"), cx)
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
                    Ok(freed) => Toast::success(format!(
                        "已清理 {}",
                        lumilio_ui::pages::settings::bytes_text(freed)
                    )),
                    Err(error) => Toast::error("没能清理游戏文件").technical(error.to_string()),
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
                    Ok(freed) => Toast::success(format!(
                        "已清理 {}",
                        lumilio_ui::pages::settings::bytes_text(freed)
                    )),
                    Err(error) => Toast::error("没能清理缓存").technical(error.to_string()),
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
                    Ok(()) => Toast::success(format!("诊断包已保存到 {}", path.display())),
                    Err(error) => Toast::error("没能导出诊断包").technical(error.to_string()),
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
            shell.toast(Toast::info("开始下载 Java，进度在动态里"), cx)
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
                    |java| format!("已安装 Java {}", java.major()),
                    "没有装上 Java".to_owned(),
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
