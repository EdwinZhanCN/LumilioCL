use super::super::panels::CONTENT_KINDS;
use super::super::{InstanceDetailView, InstanceIntent};
use super::model::{channel_label, title_of};
use crate::assets::UiIcon;
use crate::key::Key;
use crate::theme::ShellColors;
use crate::{kit, theme, tr};
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, IntoElement, Render, WeakEntity, Window, div, px,
    uniform_list,
};
use gpui_component::ActiveTheme as _;
use gpui_component::Sizable as _;
use gpui_component::StyledExt as _;
use gpui_component::WindowExt as _;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::text::TextView;
use gpui_component::{Icon, h_flex, v_flex};
use lumilio_core::{ContentEntry, Loader, ProjectKind, Version};
use std::rc::Rc;

impl InstanceDetailView {
    /// Opens the switch-version dialog for one file. With `update`, the
    /// newest compatible version is chosen at first; otherwise the current.
    pub(in super::super) fn open_switch(
        &mut self,
        entry: &ContentEntry,
        update: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(source), Some(record)) = (&entry.source, &self.record) else {
            return;
        };
        if self.busy {
            return;
        }
        let kind = CONTENT_KINDS[self.content_index()];
        let parent = cx.entity().downgrade();
        let switch = cx.new(|cx| {
            VersionSwitch::new(
                SwitchTarget {
                    kind,
                    file_name: entry.item.file_name.clone(),
                    project: source.project_id.clone(),
                    title: title_of(entry).to_owned(),
                    current: source.version_id.clone(),
                    preferred: if update {
                        entry.update.as_ref().map(|version| version.id.clone())
                    } else {
                        None
                    },
                    game_version: record.game_version.clone(),
                    loader: record.loader,
                },
                parent,
                window,
                cx,
            )
        });
        self.switch = Some(switch.clone());
        (self.handler)(
            InstanceIntent::LoadVersions(source.project_id.clone()),
            window,
            cx,
        );
        let closed = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let closed = closed.clone();
            VersionSwitch::dialog(&switch, dialog, cx).on_close(move |_, _, cx| {
                let _ = closed.update(cx, |view, cx| {
                    view.switch = None;
                    cx.notify();
                });
            })
        });
    }

    /// A project's versions arrived; only the open dialog for it takes them.
    pub fn versions_arrived(
        &mut self,
        project: &str,
        result: Result<Vec<Version>, String>,
        cx: &mut Context<Self>,
    ) {
        if let Some(switch) = &self.switch {
            switch.update(cx, |switch, cx| {
                switch.versions_arrived(project, result, cx)
            });
        }
    }
}

/// The file a switch-version dialog is for.
#[derive(Clone, Debug)]
pub struct SwitchTarget {
    pub kind: ProjectKind,
    pub file_name: String,
    pub project: String,
    pub title: String,
    /// The installed version's id.
    pub current: String,
    /// Chosen at first instead of the current one ("update" opens with the newest).
    pub preferred: Option<String>,
    pub game_version: String,
    pub loader: Loader,
}

/// P-VERSION-SWITCH: one dialog for "update" and "switch version".
pub struct VersionSwitch {
    target: SwitchTarget,
    parent: WeakEntity<InstanceDetailView>,
    versions: Option<Result<Vec<Version>, String>>,
    selected: Option<String>,
    query: Entity<InputState>,
    show_incompatible: bool,
    busy: bool,
    error: Option<String>,
    close: bool,
}

impl VersionSwitch {
    pub(super) fn new(
        target: SwitchTarget,
        parent: WeakEntity<InstanceDetailView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let query = cx.new(|cx| {
            InputState::new(window, cx).placeholder(tr!("instance-content-search-versions"))
        });
        cx.subscribe(&query, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        Self {
            target,
            parent,
            versions: None,
            selected: None,
            query,
            show_incompatible: false,
            busy: false,
            error: None,
            close: false,
        }
    }

    pub(super) fn fits(&self, version: &Version) -> bool {
        lumilio_core::version_fits(
            version,
            self.target.kind,
            &self.target.game_version,
            self.target.loader,
        )
    }

    pub(super) fn versions_arrived(
        &mut self,
        project: &str,
        result: Result<Vec<Version>, String>,
        cx: &mut Context<Self>,
    ) {
        if project != self.target.project {
            return;
        }
        if let Ok(versions) = &result {
            let wanted = self
                .target
                .preferred
                .clone()
                .unwrap_or_else(|| self.target.current.clone());
            self.selected = versions
                .iter()
                .find(|version| version.id == wanted)
                .or_else(|| versions.iter().find(|version| self.fits(version)))
                .map(|version| version.id.clone());
        }
        self.versions = Some(result);
        cx.notify();
    }

    /// The write finished: success closes, a failure stays here.
    pub(in super::super) fn finished(&mut self, failure: Option<String>, cx: &mut Context<Self>) {
        self.busy = false;
        match failure {
            Some(message) => self.error = Some(message),
            None => self.close = true,
        }
        cx.notify();
    }

    pub(super) fn chosen(&self) -> Option<&Version> {
        let Some(Ok(versions)) = &self.versions else {
            return None;
        };
        let id = self.selected.as_ref()?;
        versions.iter().find(|version| &version.id == id)
    }

    pub(super) fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(version) = self
            .chosen()
            .filter(|v| v.id != self.target.current)
            .cloned()
        else {
            return;
        };
        if self.busy {
            return;
        }
        self.busy = true;
        self.error = None;
        cx.notify();
        let target = self.target.clone();
        let _ = self.parent.update(cx, |view, cx| {
            view.send(
                InstanceIntent::SwitchContent {
                    kind: target.kind,
                    file_name: target.file_name,
                    project: target.project,
                    version_id: version.id,
                },
                window,
                cx,
            );
        });
    }

    pub(super) fn dialog(
        entity: &Entity<Self>,
        dialog: gpui_component::dialog::Dialog,
        cx: &mut App,
    ) -> gpui_component::dialog::Dialog {
        let this = entity.read(cx);
        let busy = this.busy;
        let chosen = this.chosen().cloned();
        let differs = chosen
            .as_ref()
            .is_some_and(|version| version.id != this.target.current);
        let label = match &chosen {
            Some(version) if differs => {
                tr!(
                    "instance-content-switch-to",
                    version = version.number.as_str()
                )
            }
            _ => tr!("project-switch-version").to_owned(),
        };
        let weak = entity.downgrade();
        let commit = theme::clickable(
            Key::new("switch-commit")
                .label(label)
                .primary()
                .loading(busy)
                .disabled(busy || !differs)
                .debug_selector(|| "switch-commit".into())
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |switch, cx| switch.submit(window, cx));
                }),
            !busy && differs,
        );
        let cancel = theme::clickable(
            Key::new("switch-cancel")
                .label(tr!("common-cancel"))
                .white()
                .disabled(busy)
                .on_click(|_, window, cx| window.close_dialog(cx)),
            !busy,
        );
        let colors = ShellColors::from_theme(cx.theme());
        theme::dialog(dialog, cx)
            .title(tr!(
                "instance-content-switch-title",
                title = this.target.title.as_str()
            ))
            .w(px(760.))
            .keyboard(!busy)
            .overlay_closable(!busy)
            .close_button(!busy)
            .child(entity.clone())
            .footer(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(colors.muted)
                            .child(tr!("instance-content-switch-note")),
                    )
                    .child(cancel)
                    .child(commit),
            )
    }
}

impl Render for VersionSwitch {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.close) && window.has_active_dialog(cx) {
            window.close_dialog(cx);
        }
        let colors = ShellColors::from_theme(cx.theme());
        let left: AnyElement = match &self.versions {
            None => kit::empty(tr!("version-picker-loading"), "", colors).into_any_element(),
            Some(Err(message)) => v_flex()
                .gap_2()
                .child(kit::empty(
                    tr!("new-game-read-game-versions"),
                    tr!("instance-read-versions-help"),
                    colors,
                ))
                .child(kit::technical("switch-technical", message.clone()))
                .into_any_element(),
            Some(Ok(versions)) => {
                let query = self.query.read(cx).value().trim().to_lowercase();
                let shown: Vec<(Version, bool)> = versions
                    .iter()
                    .filter(|version| {
                        query.is_empty() || version.number.to_lowercase().contains(&query)
                    })
                    .map(|version| (version.clone(), self.fits(version)))
                    .filter(|(version, fits)| {
                        *fits || self.show_incompatible || version.id == self.target.current
                    })
                    .collect();
                let current = self.target.current.clone();
                let newest_fit = versions
                    .iter()
                    .find(|version| self.fits(version))
                    .map(|v| v.id.clone());
                let selected = self.selected.clone();
                let view = cx.entity().downgrade();
                let count = shown.len();
                let shown = Rc::new(shown);
                uniform_list("switch-versions", count, move |range, _, _| {
                    range
                        .map(|index| {
                            let (version, fits) = &shown[index];
                            let id = version.id.clone();
                            let picked = selected.as_deref() == Some(id.as_str());
                            let view = view.clone();
                            let marker = if version.id == current {
                                Some(tr!("instance-content-current"))
                            } else if newest_fit.as_deref() == Some(id.as_str()) {
                                Some(tr!("instance-content-latest"))
                            } else {
                                None
                            };
                            h_flex()
                                .id(("switch-version", index))
                                .debug_selector(move || format!("switch-version-{index}"))
                                .w_full()
                                .h(px(34.))
                                .px_2()
                                .gap_2()
                                .items_center()
                                .rounded(px(6.))
                                .cursor_pointer()
                                .when(picked, |row| row.bg(colors.surface_active))
                                .hover(|row| row.bg(colors.surface_subtle))
                                .on_click(move |_, _, cx| {
                                    let id = id.clone();
                                    let _ = view.update(cx, |switch, cx| {
                                        switch.selected = Some(id);
                                        cx.notify();
                                    });
                                })
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(colors.muted)
                                        .w(px(28.))
                                        .child(channel_label(version.channel)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_sm()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .whitespace_nowrap()
                                        .text_color(if *fits {
                                            colors.foreground
                                        } else {
                                            colors.muted
                                        })
                                        .child(version.number.clone()),
                                )
                                .children(marker.map(|marker| {
                                    div().text_xs().text_color(colors.primary).child(marker)
                                }))
                                .when(!fits, |row| {
                                    row.child(
                                        div()
                                            .text_xs()
                                            .text_color(colors.danger)
                                            .child(tr!("instance-content-incompatible")),
                                    )
                                })
                        })
                        .collect()
                })
                .h(px(300.))
                .into_any_element()
            }
        };

        let toggle = {
            let view = cx.entity().downgrade();
            h_flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.muted)
                        .child(tr!("instance-content-show-incompatible")),
                )
                .child(kit::switch(
                    "switch-incompatible",
                    self.show_incompatible,
                    tr!("instance-content-show-incompatible"),
                    move |_, cx| {
                        let _ = view.update(cx, |switch, cx| {
                            switch.show_incompatible = !switch.show_incompatible;
                            cx.notify();
                        });
                    },
                ))
        };

        let right: AnyElement = match self.chosen() {
            None => div().into_any_element(),
            Some(version) => {
                let date = version.published.get(..10).unwrap_or_default().to_owned();
                let fits = self.fits(version);
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .text_base()
                                    .font_semibold()
                                    .child(version.number.clone()),
                            )
                            .child(kit::chip(channel_label(version.channel), None, colors))
                            .child(div().flex_1())
                            .child(div().text_xs().text_color(colors.muted).child(date)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(if fits { colors.muted } else { colors.danger })
                            .child({
                                let loaders = version.loaders.join(" / ");
                                let versions = version
                                    .game_versions
                                    .iter()
                                    .rev()
                                    .take(4)
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                if fits {
                                    tr!(
                                        "instance-content-version-meta",
                                        loaders = loaders,
                                        versions = versions
                                    )
                                } else {
                                    tr!(
                                        "instance-content-version-meta-incompatible",
                                        loaders = loaders,
                                        versions = versions
                                    )
                                }
                            }),
                    )
                    .child(
                        div()
                            .id("switch-changelog")
                            .h(px(260.))
                            .overflow_y_scroll()
                            .text_sm()
                            .child(if version.changelog.trim().is_empty() {
                                div()
                                    .text_color(colors.muted)
                                    .child(tr!("instance-content-no-changelog"))
                                    .into_any_element()
                            } else {
                                TextView::markdown(
                                    "switch-changelog-text",
                                    version.changelog.clone(),
                                )
                                .selectable(true)
                                .into_any_element()
                            }),
                    )
                    .into_any_element()
            }
        };

        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .w_full()
                    .gap_4()
                    .items_start()
                    .child(
                        v_flex()
                            .w(px(280.))
                            .flex_none()
                            .gap_2()
                            .child(
                                Input::new(&self.query).small().prefix(
                                    Icon::new(UiIcon::Search)
                                        .size(px(14.))
                                        .text_color(colors.muted),
                                ),
                            )
                            .child(left)
                            .child(toggle),
                    )
                    .child(v_flex().flex_1().min_w_0().child(right)),
            )
            .children(self.error.clone().map(|message| {
                div()
                    .text_sm()
                    .text_color(colors.danger)
                    .debug_selector(|| "switch-error".into())
                    .child(message)
            }))
    }
}
