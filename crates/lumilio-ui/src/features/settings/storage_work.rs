use gpui::{AppContext as _, Context, Window};

use super::settings_actions::{Settings, SettingsEvent};
use super::text::bytes_text;
use crate::collections::confirm_reclaim;
use crate::runtime;
use crate::toast::{self, Toast};
use crate::{platform, tr};

impl Settings {
    /// Measures what no game uses; if there is any, asks before removing it.
    pub(super) fn check_reclaimable(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(worker) = &self.worker else {
            return;
        };
        let service = worker.service().clone();
        let handle = worker.spawn(async move { service.reclaimable().await });
        let window = window.window_handle();
        cx.spawn(async move |this, cx| {
            let result = handle.await.unwrap_or_else(|error| Err(error.into()));
            match result {
                Err(error) => cx.update(|cx| {
                    toast::post(
                        Toast::error(tr!("settings-reclaim-failed")).technical(error.to_string()),
                        cx,
                    );
                }),
                Ok(found) if found.unused.is_empty() => cx.update(|cx| {
                    toast::post(Toast::info(tr!("settings-reclaim-none")), cx);
                }),
                Ok(found) => {
                    let size = bytes_text(found.bytes());
                    let _ = cx.update_window(window, |_, window, cx| {
                        confirm_reclaim(
                            &size,
                            &found.kept,
                            move |_, cx| {
                                let _ = this.update(cx, |this, cx| this.reclaim(cx));
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
    pub(super) fn reclaim(&mut self, cx: &mut Context<Self>) {
        let Some(worker) = &self.worker else {
            return;
        };
        let service = worker.service().clone();
        runtime::job(
            worker,
            cx,
            None::<SettingsEvent>,
            None,
            async move { service.reclaim().await },
            |this, result, cx| {
                toast::post(
                    match result {
                        Ok(freed) => {
                            Toast::success(tr!("settings-cleaned", size = bytes_text(freed)))
                        }
                        Err(error) => Toast::error(tr!("settings-clean-files-failed"))
                            .technical(error.to_string()),
                    },
                    cx,
                );
                // The disk is measured again.
                this.load(cx);
            },
        );
    }

    /// Removes the unpacked natives and `cache/`; the next launch makes them
    /// again.
    pub(super) fn clear_cache(&mut self, cx: &mut Context<Self>) {
        let Some(worker) = &self.worker else {
            return;
        };
        let service = worker.service().clone();
        runtime::job(
            worker,
            cx,
            None::<SettingsEvent>,
            None,
            async move { service.clear_cache().await },
            |this, result, cx| {
                toast::post(
                    match result {
                        Ok(freed) => {
                            Toast::success(tr!("settings-cleaned", size = bytes_text(freed)))
                        }
                        Err(error) => Toast::error(tr!("settings-clear-cache-failed"))
                            .technical(error.to_string()),
                    },
                    cx,
                );
                this.load(cx);
            },
        );
    }

    /// Removes the seed maps worked out so far.
    pub(super) fn clear_map_cache(&mut self, cx: &mut Context<Self>) {
        let Some(worker) = &self.worker else {
            return;
        };
        let service = worker.service().clone();
        runtime::job(
            worker,
            cx,
            None::<SettingsEvent>,
            None,
            async move { service.clear_map_cache().await },
            |_, result, cx| {
                toast::post(
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

    /// Asks where to save the diagnostics bundle, then writes it there.
    pub(super) fn export_diagnostics(&mut self, cx: &mut Context<Self>) {
        let Some(worker) = self.worker.clone() else {
            return;
        };
        let start = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| worker.service().layout().root().to_owned());
        let chosen = platform::pick_save_path(cx, &start, "lumilio-diagnostics.zip");
        cx.spawn(async move |_, cx| {
            let Some(path) = chosen.await else {
                return;
            };
            let service = worker.service().clone();
            let target = path.clone();
            let result = worker
                .spawn(async move { service.export_diagnostics(&target).await })
                .await
                .unwrap_or_else(|error| Err(error.into()));
            let told = match result {
                Ok(()) => Toast::success(tr!(
                    "settings-diagnostics-saved",
                    path = path.display().to_string()
                )),
                Err(error) => {
                    Toast::error(tr!("settings-diagnostics-failed")).technical(error.to_string())
                }
            };
            cx.update(|cx| toast::post(told, cx));
        })
        .detach();
    }
}
