use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;

use gpui::{AnyWindowHandle, AppContext as _, Context, Window};
use lumilio_core::{WallpaperChoice, import_wallpaper};

use super::settings_actions::Settings;
use crate::plugin_setting_dialog::PluginSettingDialog;
use crate::runtime;
use crate::theme::{Catalog, scan_dir};
use crate::toast::{self, Toast};
use crate::{platform, tr};

impl Settings {
    /// Copies the chosen picture into the launcher's folder off the UI
    /// thread, then saves it as the wallpaper with the other preferences.
    pub(super) fn set_wallpaper(
        &mut self,
        source: PathBuf,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let Some(worker) = &self.worker else {
            return;
        };
        let dir = worker.service().layout().themes();
        let import = worker.spawn(async move {
            tokio::task::spawn_blocking(move || import_wallpaper(&source, &dir)).await
        });
        cx.spawn(async move |this, cx| {
            let failure = match import.await {
                Ok(Ok(Ok(name))) => {
                    let _ = this.update(cx, |this, cx| {
                        let Some(view) = &this.view else {
                            return;
                        };
                        let mut next = view.preferences.clone();
                        next.look.wallpaper = WallpaperChoice::Picture(name);
                        this.save_preferences(next, window, cx);
                    });
                    return;
                }
                Ok(Ok(Err(error))) => error.to_string(),
                Ok(Err(error)) => error.to_string(),
                Err(error) => error.to_string(),
            };
            cx.update(|cx| {
                toast::post(
                    Toast::error(tr!("settings-wallpaper-failed")).technical(failure),
                    cx,
                );
            });
        })
        .detach();
    }

    /// Reads the local theme files off the UI thread and applies the look
    /// again once they are in, so a theme chosen earlier shows as soon as it
    /// is found. The application also asks for this at start-up.
    pub fn reload_themes(&mut self, window: AnyWindowHandle, cx: &mut Context<Self>) {
        let Some(worker) = &self.worker else {
            return;
        };
        let themes = worker.service().layout().themes();
        let scan = worker
            .spawn(async move { tokio::task::spawn_blocking(move || scan_dir(&themes)).await });
        cx.spawn(async move |_, cx| {
            if let Ok(Ok((entries, rejected))) = scan.await {
                let _ = cx.update_window(window, |_, window, cx| {
                    platform::apply_catalog(Catalog::with_local(entries, rejected), window, cx);
                });
            }
        })
        .detach();
    }

    /// Opens the dialog for a plugin's text or number setting, with its
    /// saved value. Saving keeps the draft until the launcher has it.
    pub(super) fn edit_plugin_setting(
        &mut self,
        id: String,
        key: String,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Some(worker) = self.worker.clone() else {
            return;
        };
        let service = worker.service().clone();
        let (lookup_id, lookup_key) = (id.clone(), key.clone());
        let lookup = worker.spawn(async move {
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
        let window = window.window_handle();
        cx.spawn(async move |this, cx| {
            let Some(Some((field, value))) = runtime::answer(lookup, cx).await else {
                return;
            };
            let save = Rc::new(move |value| {
                let service = worker.service().clone();
                let (id, key) = (id.clone(), key.clone());
                let task =
                    worker.spawn(async move { service.set_plugin_value(&id, &key, value).await });
                let pending: Pin<Box<dyn Future<Output = Result<(), String>>>> =
                    Box::pin(async move {
                        match task.await {
                            Ok(Ok(())) => Ok(()),
                            _ => Err(tr!("plugin-setting-save-failed").to_owned()),
                        }
                    });
                pending
            });
            let saved = Rc::new(move |cx: &mut gpui::App| {
                let _ = this.update(cx, |this, cx| this.load(cx));
            });
            let _ = cx.update_window(window, |_, window, cx| {
                PluginSettingDialog::open(field, value, save, saved, window, cx);
            });
        })
        .detach();
    }
}
