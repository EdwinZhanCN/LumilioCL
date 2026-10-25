use std::future::Future;

use gpui::{AnyWindowHandle, Context, EventEmitter, Window};
use lumilio_core::{PluginInfo, Preferences, ServiceError};

use super::page::{SettingsChoices, SettingsControls, SettingsInputs};
use crate::live::{LiveHandler, LiveIntent, SettingsView, settings_view};
use crate::runtime::{self, Worker};
use crate::toast::{self, Toast};
use crate::{platform, tr};

/// What Settings tells the shell and the application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SettingsEvent {
    /// New preferences were saved. The application applies them (look,
    /// language, motion, translation) in `window` and keeps them for what it
    /// saves with them.
    PreferencesSaved {
        preferences: Box<Preferences>,
        window: AnyWindowHandle,
    },
    /// Automatic updates were turned on or off; the updater follows.
    AutoUpdate(bool),
    /// The person asked for an update check now; the updater does it.
    CheckUpdates,
    /// Work started here shows in Activity: the lists are read again.
    TasksChanged,
    /// The plugins were read: the shell tells the game pages which are on.
    PluginsRead(Vec<PluginInfo>),
}

/// The Settings page: its tab, the settings it reads and shows, and the
/// work of saving each one.
pub struct Settings {
    pub(super) worker: Option<Worker>,
    /// What the page shows of the rest of the launcher.
    pub(super) inputs: SettingsInputs,
    pub(super) choices: SettingsChoices,
    /// The settings last read; `None` until they are.
    pub(super) view: Option<SettingsView>,
    pub(super) controls: Option<SettingsControls>,
    /// Where the page sends what it asks for.
    pub(super) handler: Option<LiveHandler>,
}

impl EventEmitter<SettingsEvent> for Settings {}

impl Settings {
    #[must_use]
    pub fn new(worker: Option<Worker>) -> Self {
        Self {
            worker,
            inputs: SettingsInputs::default(),
            choices: SettingsChoices::default(),
            view: None,
            controls: None,
            handler: None,
        }
    }

    /// Gives the feature the launcher to ask (the application does, once).
    pub fn set_worker(&mut self, worker: Worker) {
        self.worker = Some(worker);
    }

    /// Reads the settings, the plugins and the Java list, then how the disk
    /// is used. The page reads them each time it is shown.
    pub fn load(&mut self, cx: &mut Context<Self>) {
        let Some(worker) = self.worker.clone() else {
            return;
        };
        let service = worker.service().clone();
        let handle = worker.spawn(async move {
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
        cx.spawn(async move |this, cx| {
            let Some((settings, plugins, java, memory, root)) = runtime::answer(handle, cx).await
            else {
                return;
            };
            let read = this.update(cx, |this, cx| {
                // What is measured or fetched separately stays until it is again.
                let storage = this.view.as_ref().and_then(|view| view.storage);
                let offers = this
                    .view
                    .as_ref()
                    .map(|view| view.java_offers.clone())
                    .unwrap_or_default();
                let offers_known =
                    !offers.is_empty() && offers.iter().all(|(_, majors)| !majors.is_empty());
                let mut view = settings_view(&settings, &java, storage, &root, memory);
                view.plugins = plugins.clone();
                view.java_offers = offers;
                this.view = Some(view);
                cx.emit(SettingsEvent::PluginsRead(plugins));
                cx.notify();
                if !offers_known {
                    this.load_java_offers(cx);
                }
            });
            if read.is_err() {
                return;
            }
            let service = worker.service().clone();
            let handle = worker.spawn(async move { service.storage_usage().await });
            let Some(usage) = runtime::answer(handle, cx).await else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                if let Some(view) = &mut this.view {
                    view.storage = Some(usage);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Saves one setting. A failure is told with its raw cause; success says
    /// `notice` (when there is one) and runs `after`. Either way the
    /// settings are read again, so the page shows what was kept.
    pub(super) fn save(
        &mut self,
        work: impl Future<Output = Result<(), ServiceError>> + Send + 'static,
        failed: &'static str,
        notice: Option<&'static str>,
        after: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(worker) = &self.worker else {
            return;
        };
        runtime::job(
            worker,
            cx,
            None::<SettingsEvent>,
            None,
            work,
            move |this, result, cx| {
                match result {
                    Ok(()) => {
                        if let Some(notice) = notice {
                            toast::post(Toast::success(notice), cx);
                        }
                        after(this, cx);
                    }
                    Err(error) => {
                        toast::post(Toast::error(failed).technical(error.to_string()), cx);
                    }
                }
                this.load(cx);
            },
        );
    }

    /// Saves the launcher-wide preferences; the application applies them.
    pub(super) fn save_preferences(
        &mut self,
        preferences: Preferences,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.worker.as_ref().map(|worker| worker.service().clone()) else {
            return;
        };
        let saved = preferences.clone();
        self.save(
            async move { service.set_preferences(saved).await },
            tr!("settings-save-failed"),
            None,
            move |_, cx| {
                cx.emit(SettingsEvent::PreferencesSaved {
                    preferences: Box::new(preferences),
                    window,
                })
            },
            cx,
        );
    }

    /// Does what the page asked for. The shell sends only this page's
    /// intents here; any other is ignored.
    pub fn answer(&mut self, intent: LiveIntent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(service) = self.worker.as_ref().map(|worker| worker.service().clone()) else {
            return;
        };
        let failed = tr!("settings-save-failed");
        let next_launch = Some(tr!("settings-saved-next-launch"));
        match intent {
            LiveIntent::LoadSettings => self.load(cx),
            LiveIntent::CheckUpdates => cx.emit(SettingsEvent::CheckUpdates),
            LiveIntent::SetAutoUpdate(enabled) => self.save(
                async move { service.set_auto_update(enabled).await },
                failed,
                None,
                move |_, cx| cx.emit(SettingsEvent::AutoUpdate(enabled)),
                cx,
            ),
            LiveIntent::SetPreferences(preferences) => {
                self.save_preferences(preferences, window.window_handle(), cx);
            }
            LiveIntent::SetLaunchDefaults(launch) => self.save(
                async move { service.set_launch_defaults(launch).await },
                failed,
                next_launch,
                |_, _| {},
                cx,
            ),
            LiveIntent::SetMemory { min_mb, max_mb } => self.save(
                async move { service.set_default_memory(min_mb, max_mb).await },
                tr!("settings-memory-save-failed"),
                next_launch,
                |_, _| {},
                cx,
            ),
            LiveIntent::SetConcurrency(count) => self.save(
                async move { service.set_download_concurrency(count).await },
                failed,
                Some(tr!("settings-saved-next-download")),
                |_, _| {},
                cx,
            ),
            LiveIntent::AddMirrorPreset(preset) => self.save(
                async move { service.add_mirror_preset(preset).await },
                tr!("settings-mirror-add-failed"),
                None,
                |_, _| {},
                cx,
            ),
            LiveIntent::SetMirrors(mirrors) => self.save(
                async move { service.set_mirror_rules(mirrors).await },
                tr!("settings-mirrors-save-failed"),
                None,
                |_, _| {},
                cx,
            ),
            LiveIntent::SetDownloadSource(preference) => self.save(
                async move { service.set_download_source(preference).await },
                tr!("settings-download-source-save-failed"),
                None,
                |_, _| {},
                cx,
            ),
            LiveIntent::SetAutoDownloadBlockAssets(enabled) => self.save(
                async move { service.set_auto_download_block_assets(enabled).await },
                failed,
                None,
                |_, _| {},
                cx,
            ),
            LiveIntent::SetJavaRoots(roots) => self.save(
                async move { service.set_java_roots(roots).await },
                tr!("settings-java-roots-save-failed"),
                None,
                |_, _| {},
                cx,
            ),
            LiveIntent::SetJavaDisabled { home, disabled } => self.save(
                async move { service.set_java_disabled(&home, disabled).await },
                tr!("settings-java-save-failed"),
                None,
                |_, _| {},
                cx,
            ),
            LiveIntent::SetJavaSource(source) => self.save(
                async move { service.set_java_source(source).await },
                tr!("settings-java-save-failed"),
                None,
                |_, _| {},
                cx,
            ),
            LiveIntent::SetAutoInstallJava(enabled) => self.save(
                async move { service.set_auto_install_java(enabled).await },
                tr!("settings-java-save-failed"),
                None,
                |_, _| {},
                cx,
            ),
            LiveIntent::RemoveJava(home) => self.save(
                async move { service.remove_java(&home).await },
                tr!("settings-java-remove-failed"),
                Some(tr!("settings-java-removed")),
                |_, _| {},
                cx,
            ),
            LiveIntent::AddJava => self.add_java(cx),
            LiveIntent::DownloadJava { source, major } => self.download_java(source, major, cx),
            LiveIntent::SetPluginEnabled { id, enabled } => self.save(
                async move { service.set_plugin_enabled(&id, enabled).await },
                tr!("settings-plugin-save-failed"),
                None,
                |_, _| {},
                cx,
            ),
            LiveIntent::SetPluginValue { id, key, value } => self.save(
                async move { service.set_plugin_value(&id, &key, value).await },
                tr!("settings-plugin-save-failed"),
                None,
                |_, _| {},
                cx,
            ),
            LiveIntent::ResetPlugin(id) => self.save(
                async move { service.reset_plugin(&id).await },
                tr!("settings-plugin-reset-failed"),
                None,
                |_, _| {},
                cx,
            ),
            LiveIntent::EditPluginSetting { id, key } => {
                self.edit_plugin_setting(id, key, window, cx);
            }
            LiveIntent::ReloadThemes => self.reload_themes(window.window_handle(), cx),
            LiveIntent::SetWallpaper(path) => self.set_wallpaper(path, window.window_handle(), cx),
            LiveIntent::Reveal(path) => platform::reveal(&path, cx),
            LiveIntent::ClearCache | LiveIntent::ClearTranslationCache => self.clear_cache(cx),
            LiveIntent::ClearMapCache => self.clear_map_cache(cx),
            LiveIntent::CheckReclaimable => self.check_reclaimable(window, cx),
            LiveIntent::Reclaim => self.reclaim(cx),
            LiveIntent::ExportDiagnostics => self.export_diagnostics(cx),
            _ => {}
        }
    }
}
