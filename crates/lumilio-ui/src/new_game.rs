//! The new-game dialog (IA `library.md#新建游戏`): name, loader, game
//! version (P-VERSION), loader version (P-LOADER-VERSION) and whether to
//! download the game files at once. It holds no business logic: metadata and
//! the creation itself go through [`NewGameIntent`]s and come back through
//! [`NewGameForm::game_versions_arrived`] and friends.

use std::rc::Rc;

use crate::controls::Checkbox;
use crate::key::Key;
use gpui::{App, Context, Entity, IntoElement, Render, WeakEntity, Window, div, prelude::*, px};
use gpui_component::dialog::Dialog;
use gpui_component::input::{Input, InputState};
use gpui_component::{
    ActiveTheme as _, Sizable as _, StyledExt as _, WindowExt as _, h_flex, v_flex,
};
use lumilio_core::{CatalogEntry, Loader, LoaderVersion, VersionChannel};

use crate::kit;
use crate::theme::{self, ShellColors};
use crate::version_picker::{Choice, Choices, PickerEvent, VersionPicker};
use crate::{tr, tr_all};

/// The loaders offered, in the order shown.
pub const LOADERS: [Loader; 5] = [
    Loader::Vanilla,
    Loader::Fabric,
    Loader::NeoForge,
    Loader::Forge,
    Loader::Quilt,
];
fn loader_labels() -> [&'static str; 5] {
    [
        tr!("loader-vanilla"),
        "Fabric",
        "NeoForge",
        "Forge",
        "Quilt",
    ]
}

fn loader_modes() -> &'static [&'static str] {
    tr_all![
        "new-game-loader-stable",
        "new-game-loader-latest",
        "new-game-loader-other",
    ]
}

/// The dialog is 480 wide with 16 of padding each side; the popover adds 12.
const DIALOG_WIDTH: f32 = 480.;
const PICKER_MENU_WIDTH: gpui::Pixels = px(DIALOG_WIDTH - 32. - 24.);

pub fn install_help() -> &'static str {
    tr!("new-game-install-help")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewGameRequest {
    pub name: String,
    pub loader: Loader,
    pub game_version: String,
    /// `None` only for vanilla.
    pub loader_version: Option<String>,
    /// Download the game files right after creating.
    pub install: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NewGameIntent {
    /// Answer with [`NewGameForm::game_versions_arrived`].
    LoadGameVersions,
    /// Answer with [`NewGameForm::loader_versions_arrived`].
    LoadLoaderVersions {
        loader: Loader,
        game_version: String,
    },
    /// Answer with [`NewGameForm::created`].
    Create(NewGameRequest),
}

pub type NewGameHandler = Rc<dyn Fn(NewGameIntent, &mut Window, &mut App)>;

/// Which loader version is wanted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoaderPick {
    Stable,
    Latest,
    Other,
}

/// What a failure looks like: one sentence, and the raw cause behind 技术详情.
pub type Failure = (String, String);

fn channel_tag(channel: VersionChannel) -> Option<&'static str> {
    match channel {
        VersionChannel::Release => None,
        VersionChannel::Snapshot => Some(tr!("new-game-channel-snapshot")),
        VersionChannel::PreRelease => Some(tr!("new-game-channel-pre-release")),
        VersionChannel::Candidate => Some(tr!("new-game-channel-candidate")),
        VersionChannel::Old => Some(tr!("new-game-channel-old")),
    }
}

/// Game versions as picker choices: releases are shown by default.
#[must_use]
pub fn game_choices(entries: &[CatalogEntry]) -> Vec<Choice> {
    entries
        .iter()
        .map(|entry| {
            let channel = entry.channel();
            Choice {
                id: entry.id().to_owned(),
                primary: channel == VersionChannel::Release,
                tag: channel_tag(channel),
            }
        })
        .collect()
}

/// Loader versions as picker choices: all shown, each marked stable or not.
#[must_use]
pub fn loader_choices(versions: &[LoaderVersion]) -> Vec<Choice> {
    versions
        .iter()
        .map(|version| Choice {
            id: version.version.clone(),
            primary: true,
            tag: Some(if version.stable {
                tr!("new-game-loader-tag-stable")
            } else {
                tr!("new-game-loader-tag-testing")
            }),
        })
        .collect()
}

/// The loader version a pick resolves to, if there is one.
#[must_use]
pub fn resolve(
    pick: LoaderPick,
    versions: &[LoaderVersion],
    other: Option<&str>,
) -> Option<String> {
    match pick {
        LoaderPick::Stable => lumilio_core::recommended_loader(versions).map(|v| v.version.clone()),
        LoaderPick::Latest => versions.first().map(|v| v.version.clone()),
        LoaderPick::Other => other
            .filter(|chosen| versions.iter().any(|v| v.version == *chosen))
            .map(str::to_owned),
    }
}

/// The name used when the field is left empty: loader and game version.
#[must_use]
pub fn default_name(loader: Loader, game_version: Option<&str>) -> String {
    let label = loader_labels()[LOADERS.iter().position(|l| *l == loader).unwrap_or(0)];
    match game_version {
        Some(version) => format!("{label} {version}"),
        None => tr!("new-game-default-name", loader = label),
    }
}

type LoaderList = Option<Result<Vec<LoaderVersion>, Failure>>;

/// What an existing game is on now, when the form changes it instead of
/// creating a new one.
/// Something the dialog can ask the caller to do, with no input.
pub type SnapshotAction = Rc<dyn Fn(&mut Window, &mut App)>;

pub struct RuntimeChange {
    pub game_version: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
    /// Makes a snapshot of the game before anything changes.
    pub snapshot: SnapshotAction,
}

pub fn change_warning() -> &'static str {
    tr!("new-game-change-warning")
}

pub struct NewGameForm {
    handler: NewGameHandler,
    /// Set when this form changes an existing game's version and loader.
    change: Option<RuntimeChange>,
    /// The current loader version has been offered once as the starting choice.
    started_on_current: bool,
    name: Entity<InputState>,
    placeholder: String,
    loader: Loader,
    game: Entity<VersionPicker>,
    pick: LoaderPick,
    other: Entity<VersionPicker>,
    /// The loader versions for `(loader, game version)`; a late answer for
    /// another pair is ignored.
    loader_key: Option<(Loader, String)>,
    loader_versions: LoaderList,
    install: bool,
    busy: bool,
    error: Option<Failure>,
    /// A finished creation closes the dialog on the next render.
    close: bool,
}

impl NewGameForm {
    pub fn new(handler: NewGameHandler, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx));
        let game = cx.new(|cx| {
            VersionPicker::new(
                "new-game-version",
                true,
                tr!("new-game-pick-game-version"),
                window,
                cx,
            )
            .menu_width(PICKER_MENU_WIDTH)
        });
        let other = cx.new(|cx| {
            VersionPicker::new(
                "new-game-loader-version",
                false,
                tr!("new-game-pick-loader-version"),
                window,
                cx,
            )
            .menu_width(PICKER_MENU_WIDTH)
        });
        cx.subscribe_in(&game, window, |form, _, event: &PickerEvent, window, cx| {
            if matches!(event, PickerEvent::Retry) {
                form.request_game_versions(window, cx);
            }
            // A new game version is picked up by the next render.
            cx.notify();
        })
        .detach();
        cx.subscribe(&other, |form, _, event: &PickerEvent, cx| {
            if matches!(event, PickerEvent::Retry) {
                form.loader_key = None;
            }
            cx.notify();
        })
        .detach();
        Self {
            handler,
            change: None,
            started_on_current: false,
            name,
            placeholder: String::new(),
            loader: Loader::Fabric,
            game,
            pick: LoaderPick::Stable,
            other,
            loader_key: None,
            loader_versions: None,
            install: true,
            busy: false,
            error: None,
            close: false,
        }
    }

    /// A form that changes an existing game: it starts on what the game is on.
    pub fn for_change(
        handler: NewGameHandler,
        change: RuntimeChange,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut form = Self::new(handler, window, cx);
        form.loader = change.loader;
        form.change = Some(change);
        form
    }

    /// Asks for the game versions; call once after opening.
    pub fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.request_game_versions(window, cx);
    }

    fn request_game_versions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.game.update(cx, |picker, cx| {
            picker.set_choices(Choices::Loading, None, cx)
        });
        (self.handler)(NewGameIntent::LoadGameVersions, window, cx);
    }

    fn game_version(&self, cx: &App) -> Option<String> {
        self.game.read(cx).selected().map(str::to_owned)
    }

    /// Asks for the loader versions of the current pair, unless they are
    /// already here or on their way. Called from render, so whatever changed
    /// the pair (loader, game version, retry) needs no window of its own.
    fn request_loader_versions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(game_version) = self.game_version(cx) else {
            return;
        };
        if self.loader == Loader::Vanilla {
            return;
        }
        let key = (self.loader, game_version.clone());
        if self.loader_key.as_ref() == Some(&key) {
            return;
        }
        self.loader_key = Some(key);
        self.loader_versions = None;
        self.other.update(cx, |picker, cx| {
            picker.set_choices(Choices::Loading, None, cx)
        });
        (self.handler)(
            NewGameIntent::LoadLoaderVersions {
                loader: self.loader,
                game_version,
            },
            window,
            cx,
        );
    }

    pub fn game_versions_arrived(
        &mut self,
        result: Result<Vec<CatalogEntry>, Failure>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(entries) => {
                let latest = match &self.change {
                    Some(change) => Some(change.game_version.clone()),
                    None => entries
                        .iter()
                        .find(|entry| entry.channel() == VersionChannel::Release)
                        .map(|entry| entry.id().to_owned()),
                };
                let choices = game_choices(&entries);
                self.game.update(cx, |picker, cx| {
                    picker.set_choices(Choices::Ready(choices), latest, cx)
                });
            }
            Err((message, _)) => self.game.update(cx, |picker, cx| {
                picker.set_choices(Choices::Failed(message), None, cx)
            }),
        }
        cx.notify();
    }

    pub fn loader_versions_arrived(
        &mut self,
        loader: Loader,
        game_version: &str,
        result: Result<Vec<LoaderVersion>, Failure>,
        cx: &mut Context<Self>,
    ) {
        if self.loader_key.as_ref() != Some(&(loader, game_version.to_owned())) {
            return;
        }
        let picker = match &result {
            Ok(versions) => Choices::Ready(loader_choices(versions)),
            Err((message, _)) => Choices::Failed(message.clone()),
        };
        let mut fallback = result
            .as_ref()
            .ok()
            .and_then(|versions| lumilio_core::recommended_loader(versions))
            .map(|version| version.version.clone());
        // A change starts on the version the game is on now, shown as "其他".
        if !self.started_on_current
            && let (Some(change), Ok(versions)) = (&self.change, &result)
            && change.loader == loader
            && change.game_version == game_version
            && let Some(current) = change
                .loader_version
                .as_ref()
                .filter(|current| versions.iter().any(|v| &v.version == *current))
        {
            fallback = Some(current.clone());
            self.pick = LoaderPick::Other;
            self.started_on_current = true;
        }
        self.other
            .update(cx, |other, cx| other.set_choices(picker, fallback, cx));
        self.loader_versions = Some(result);
        cx.notify();
    }

    /// The creation finished. Success closes the dialog; a failure stays in it.
    pub fn created(&mut self, result: Result<(), Failure>, cx: &mut Context<Self>) {
        self.busy = false;
        match result {
            Ok(()) => self.close = true,
            Err(failure) => self.error = Some(failure),
        }
        cx.notify();
    }

    fn set_loader(&mut self, index: usize, cx: &mut Context<Self>) {
        let loader = LOADERS[index.min(LOADERS.len() - 1)];
        if loader == self.loader {
            return;
        }
        self.loader = loader;
        self.pick = LoaderPick::Stable;
        self.error = None;
        cx.notify();
    }

    /// The finished request, or `None` while something is missing.
    pub fn request(&self, cx: &App) -> Option<NewGameRequest> {
        let game_version = self.game_version(cx)?;
        let loader_version = if self.loader == Loader::Vanilla {
            None
        } else {
            let Some(Ok(versions)) = &self.loader_versions else {
                return None;
            };
            Some(resolve(
                self.pick,
                versions,
                self.other.read(cx).selected(),
            )?)
        };
        if let Some(change) = &self.change {
            // Nothing to do when it is the combination the game is on.
            if change.loader == self.loader
                && change.game_version == game_version
                && change.loader_version == loader_version
            {
                return None;
            }
            return Some(NewGameRequest {
                name: String::new(),
                loader: self.loader,
                game_version,
                loader_version,
                install: false,
            });
        }
        let typed = self.name.read(cx).value().trim().to_owned();
        Some(NewGameRequest {
            name: if typed.is_empty() {
                default_name(self.loader, Some(&game_version))
            } else {
                typed
            },
            loader: self.loader,
            game_version,
            loader_version,
            install: self.install,
        })
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(request) = self.request(cx) else {
            return;
        };
        self.busy = true;
        self.error = None;
        self.game
            .update(cx, |picker, cx| picker.set_disabled(true, cx));
        self.other
            .update(cx, |picker, cx| picker.set_disabled(true, cx));
        cx.notify();
        // The click is still updating this form; a handler that answers by
        // updating it again (a change finishes itself) must run after.
        let handler = self.handler.clone();
        window.defer(cx, move |window, cx| {
            handler(NewGameIntent::Create(request), window, cx)
        });
    }

    /// What the loader version row says will be installed, if known.
    fn resolved_note(&self, cx: &App) -> Option<String> {
        let Some(Ok(versions)) = &self.loader_versions else {
            return None;
        };
        resolve(self.pick, versions, self.other.read(cx).selected()).map(|version| {
            tr!(
                "new-game-resolved-note",
                loader = self.loader_label(),
                version = version
            )
        })
    }

    fn loader_label(&self) -> &'static str {
        loader_labels()[LOADERS.iter().position(|l| *l == self.loader).unwrap_or(0)]
    }

    /// Opens the dialog around `form`.
    pub fn open(form: Entity<Self>, window: &mut Window, cx: &mut App) {
        form.update(cx, |form, cx| form.start(window, cx));
        window.open_dialog(cx, move |dialog, _, cx| Self::dialog(&form, dialog, cx));
    }

    fn dialog(form: &Entity<Self>, dialog: Dialog, cx: &mut App) -> Dialog {
        let this = form.read(cx);
        let busy = this.busy;
        let ready = this.request(cx).is_some();
        let changing = this.change.is_some();
        let weak = form.downgrade();
        let create = theme::clickable(
            Key::new("new-game-create")
                .label(if changing {
                    tr!("new-game-change")
                } else {
                    tr!("new-game-create")
                })
                .primary()
                .disabled(busy || !ready)
                .loading(busy)
                .debug_selector(|| "new-game-create".into())
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |form, cx| form.submit(window, cx));
                }),
            !busy && ready,
        );
        let cancel = theme::clickable(
            Key::new("new-game-cancel")
                .label(tr!("common-cancel"))
                .white()
                .disabled(busy)
                .on_click(|_, window, cx| window.close_dialog(cx)),
            !busy,
        );
        theme::dialog(dialog, cx)
            .title(if changing {
                tr!("new-game-change-title")
            } else {
                tr!("library-new-game")
            })
            .w(px(DIALOG_WIDTH))
            .keyboard(!busy)
            .overlay_closable(!busy)
            .close_button(!busy)
            .child(form.clone())
            .footer(
                h_flex()
                    .w_full()
                    .justify_end()
                    .gap_2()
                    .child(cancel)
                    .child(create),
            )
    }
}

fn label(text: &'static str, colors: ShellColors) -> impl IntoElement {
    div()
        .text_sm()
        .font_medium()
        .text_color(colors.foreground)
        .child(text)
}

impl Render for NewGameForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.close) && window.has_active_dialog(cx) {
            window.close_dialog(cx);
        }
        if !self.busy {
            self.request_loader_versions(window, cx);
        }
        let colors = ShellColors::from_theme(cx.theme());
        let placeholder = default_name(self.loader, self.game_version(cx).as_deref());
        if placeholder != self.placeholder {
            self.placeholder = placeholder.clone();
            self.name
                .update(cx, |name, cx| name.set_placeholder(placeholder, window, cx));
        }
        let busy = self.busy;
        let entity: WeakEntity<Self> = cx.entity().downgrade();

        let loader_index = LOADERS.iter().position(|l| *l == self.loader).unwrap_or(0);
        let labels = loader_labels();
        let loaders = kit::segments("new-game-loader", &labels, loader_index, {
            let entity = entity.clone();
            move |index, _, cx| {
                let _ = entity.update(cx, |form, cx| form.set_loader(index, cx));
            }
        });

        let unsupported =
            matches!(&self.loader_versions, Some(Ok(versions)) if versions.is_empty());
        let loader_section = (self.loader != Loader::Vanilla).then(|| {
            let mode = match self.pick {
                LoaderPick::Stable => 0,
                LoaderPick::Latest => 1,
                LoaderPick::Other => 2,
            };
            let picks = kit::segments("new-game-loader-pick", loader_modes(), mode, {
                let entity = entity.clone();
                move |index, _, cx| {
                    let _ = entity.update(cx, |form, cx| {
                        form.pick = [LoaderPick::Stable, LoaderPick::Latest, LoaderPick::Other]
                            [index.min(2)];
                        cx.notify();
                    });
                }
            });
            let ready = matches!(&self.loader_versions, Some(Ok(_)));
            let body = if unsupported {
                div()
                    .text_sm()
                    .text_color(colors.muted)
                    .debug_selector(|| "new-game-unsupported".into())
                    .child(tr!(
                        "new-game-loader-unsupported",
                        loader = self.loader_label()
                    ))
                    .into_any_element()
            } else {
                // While loading or after a failure the picker shows that
                // state (with its retry), whichever pick is chosen.
                v_flex()
                    .gap_2()
                    .child(h_flex().child(picks))
                    .when(self.pick == LoaderPick::Other || !ready, |body| {
                        body.child(self.other.clone())
                    })
                    .children(self.resolved_note(cx).map(|note| {
                        div()
                            .text_xs()
                            .text_color(colors.muted)
                            .debug_selector(|| "new-game-resolved".into())
                            .child(note)
                    }))
                    .into_any_element()
            };
            v_flex()
                .gap_2()
                .child(label(tr!("new-game-loader-version-label"), colors))
                .child(body)
        });

        let install_toggle = {
            let entity = entity.clone();
            h_flex()
                .gap_1()
                .items_center()
                .child(
                    Checkbox::new("new-game-install")
                        .checked(self.install)
                        .disabled(busy)
                        .label(tr!("new-game-install-now"))
                        .on_click(move |_, _, cx| {
                            let _ = entity.update(cx, |form, cx| {
                                form.install = !form.install;
                                cx.notify();
                            });
                        }),
                )
                .child(kit::info("new-game-install-info", install_help()))
        };

        let changing = self.change.is_some();
        let snapshot = self.change.as_ref().map(|change| change.snapshot.clone());
        v_flex()
            .id("new-game-form")
            .w_full()
            .gap_4()
            .children((!changing).then(|| {
                v_flex()
                    .gap_2()
                    .child(label(tr!("common-name"), colors))
                    .child(Input::new(&self.name).disabled(busy))
            }))
            .child(
                v_flex()
                    .gap_2()
                    .child(label(tr!("library-loader"), colors))
                    .child(h_flex().child(loaders)),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(label(tr!("discover-section-version"), colors))
                    .child(self.game.clone()),
            )
            .children(loader_section)
            .children((!changing).then_some(install_toggle))
            .children(snapshot.map(|snapshot| {
                v_flex()
                    .gap_2()
                    .debug_selector(|| "new-game-change-warning".into())
                    .child(
                        div()
                            .text_sm()
                            .text_color(colors.muted)
                            .child(change_warning()),
                    )
                    .child(
                        h_flex().child(
                            kit::ghost(
                                "new-game-snapshot",
                                tr!("new-game-snapshot-first"),
                                move |window, cx| snapshot(window, cx),
                            )
                            .debug_selector(|| "new-game-snapshot".into())
                            .disabled(busy),
                        ),
                    )
            }))
            .children(self.error.clone().map(|(message, technical)| {
                h_flex()
                    .gap_2()
                    .items_center()
                    .debug_selector(|| "new-game-error".into())
                    .child(div().text_sm().text_color(colors.danger).child(message))
                    .child(kit::technical("new-game-technical", technical).xsmall())
            }))
    }
}

#[cfg(test)]
mod tests;
