use gpui::Context;
use lumilio_core::{CancellationToken, JavaBuild, JavaSource, ServiceError};

use super::settings_actions::{Settings, SettingsEvent};
use crate::runtime;
use crate::toast::{self, Toast};
use crate::{platform, tr};

impl Settings {
    /// Reads which Java majors each download source offers, for the
    /// 下载 Java… dialog. Mojang's list needs the network; until it arrives
    /// (or when it cannot) the dialog offers a fallback.
    pub(super) fn load_java_offers(&mut self, cx: &mut Context<Self>) {
        let Some(worker) = &self.worker else {
            return;
        };
        let service = worker.service().clone();
        let handle = worker.spawn(async move { service.java_offers().await });
        cx.spawn(async move |this, cx| {
            let Some(offers) = runtime::answer(handle, cx).await else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                if let Some(view) = &mut this.view {
                    view.java_offers = offers;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Asks for a Java (the program or its home folder) and adds it.
    pub(super) fn add_java(&mut self, cx: &mut Context<Self>) {
        let Some(worker) = self.worker.clone() else {
            return;
        };
        let chosen = platform::pick_path(cx, true, true, tr!("settings-java-pick"));
        cx.spawn(async move |this, cx| {
            let Some(path) = chosen.await else {
                return;
            };
            let service = worker.service().clone();
            let result = worker
                .spawn(async move { service.add_java(&path).await })
                .await
                .unwrap_or_else(|error| Err(error.into()));
            let told = match result {
                Ok(runtime) => {
                    Toast::success(tr!("settings-java-added", version = runtime.version()))
                }
                Err(ServiceError::NoJavaAt(_)) => Toast::error(tr!("settings-java-not-found")),
                Err(error) => {
                    Toast::error(tr!("settings-java-add-failed")).technical(error.to_string())
                }
            };
            cx.update(|cx| toast::post(told, cx));
            let _ = this.update(cx, |this, cx| this.load(cx));
        })
        .detach();
    }

    /// Downloads this major from this source into the launcher. Its progress
    /// shows in Activity; the Java list is read again when it ends.
    pub(super) fn download_java(&mut self, source: JavaSource, major: u32, cx: &mut Context<Self>) {
        let Some(worker) = &self.worker else {
            return;
        };
        let service = worker.service().clone();
        runtime::job(
            worker,
            cx,
            Some(SettingsEvent::TasksChanged),
            Some(Toast::info(tr!("settings-java-install-started"))),
            async move {
                service
                    .install_java_from(
                        source,
                        Some(major),
                        JavaBuild::Any,
                        CancellationToken::new(),
                    )
                    .await
            },
            |this, result, cx| {
                toast::post(
                    toast::outcome(
                        result,
                        |java| tr!("settings-java-installed", version = java.major()),
                        tr!("settings-java-install-failed").to_owned(),
                    ),
                    cx,
                );
                this.load(cx);
            },
        );
    }
}
