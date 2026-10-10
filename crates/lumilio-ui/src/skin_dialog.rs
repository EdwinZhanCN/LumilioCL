//! The skin dialog of an offline account (IA `accounts.md`): the game's
//! default, picture files on this computer, the player's LittleSkin profile,
//! or a CustomSkinLoader site. It holds no business logic: saving goes through
//! a [`SkinIntent`] and the answer comes back through [`SkinDialog::saved`].

use std::path::PathBuf;
use std::rc::Rc;

use crate::key::Key;
use gpui::{App, Context, Entity, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::dialog::Dialog;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{Sizable as _, StyledExt as _, WindowExt as _, h_flex, v_flex};
use lumilio_core::{SkinChoice, SkinModel};

use crate::kit;
use crate::new_game::Failure;
use crate::theme::{self, ShellColors};
use crate::{tr, tr_all};

const DIALOG_WIDTH: f32 = 480.;

fn kinds() -> &'static [&'static str] {
    tr_all![
        "account-skin-kind-default",
        "account-skin-kind-local",
        "account-skin-kind-littleskin",
        "account-skin-kind-site",
    ]
}

fn models() -> &'static [&'static str] {
    tr_all!["account-skin-model-classic", "account-skin-model-slim"]
}

/// What the dialog asks the application to do.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkinIntent {
    /// The account (by key).
    pub key: String,
    /// What it should look like; `None` is the game's default skin.
    pub choice: Option<SkinChoice>,
}

pub type SkinHandler = Rc<dyn Fn(SkinIntent, &mut Window, &mut App)>;

pub struct SkinDialog {
    handler: SkinHandler,
    key: String,
    name: String,
    kind: usize,
    model: usize,
    skin: Entity<InputState>,
    cape: Entity<InputState>,
    api: Entity<InputState>,
    busy: bool,
    error: Option<Failure>,
    close: bool,
    inline: bool,
}

impl SkinDialog {
    pub fn new(
        handler: SkinHandler,
        key: String,
        name: String,
        current: Option<&SkinChoice>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (mut kind, mut model) = (0, 0);
        let (mut skin, mut cape, mut api) = (String::new(), String::new(), String::new());
        match current {
            Some(SkinChoice::Local {
                model: shape,
                skin: skin_path,
                cape: cape_path,
            }) => {
                kind = 1;
                model = usize::from(*shape == SkinModel::Slim);
                skin = skin_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default();
                cape = cape_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default();
            }
            Some(SkinChoice::LittleSkin) => kind = 2,
            Some(SkinChoice::Csl { api: address }) => {
                kind = 3;
                address.clone_into(&mut api);
            }
            None => {}
        }
        let mut input = |text: String, placeholder: &'static str, cx: &mut Context<Self>| {
            cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(text)
                    .placeholder(placeholder)
            })
        };
        let skin = input(skin, tr!("account-skin-skin-placeholder"), cx);
        let cape = input(cape, tr!("account-skin-cape-placeholder"), cx);
        let api = input(api, tr!("account-skin-api-placeholder"), cx);
        for field in [&skin, &cape, &api] {
            cx.subscribe_in(field, window, |dialog, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    dialog.error = None;
                    cx.notify();
                }
            })
            .detach();
        }
        Self {
            handler,
            key,
            name,
            kind,
            model,
            skin,
            cape,
            api,
            busy: false,
            error: None,
            close: false,
            inline: false,
        }
    }

    /// The same offline choice form retained in an account's wardrobe.
    pub fn inline(mut self) -> Self {
        self.inline = true;
        self
    }

    fn text(input: &Entity<InputState>, cx: &App) -> String {
        input.read(cx).value().trim().to_owned()
    }

    /// What the form says, or why it cannot be saved yet. `Ok(None)` is the
    /// game's default skin.
    pub fn choice(&self, cx: &App) -> Result<Option<SkinChoice>, &'static str> {
        match self.kind {
            0 => Ok(None),
            1 => {
                let (skin, cape) = (Self::text(&self.skin, cx), Self::text(&self.cape, cx));
                if skin.is_empty() && cape.is_empty() {
                    return Err(tr!("account-skin-choose-picture"));
                }
                let path = |text: String| (!text.is_empty()).then(|| PathBuf::from(text));
                Ok(Some(SkinChoice::Local {
                    model: if self.model == 1 {
                        SkinModel::Slim
                    } else {
                        SkinModel::Wide
                    },
                    skin: path(skin),
                    cape: path(cape),
                }))
            }
            2 => Ok(Some(SkinChoice::LittleSkin)),
            _ => {
                let api = Self::text(&self.api, cx);
                if api.is_empty() {
                    Err(tr!("account-skin-enter-address"))
                } else {
                    Ok(Some(SkinChoice::Csl { api }))
                }
            }
        }
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Ok(choice) = self.choice(cx) else {
            return;
        };
        self.busy = true;
        self.error = None;
        cx.notify();
        (self.handler)(
            SkinIntent {
                key: self.key.clone(),
                choice,
            },
            window,
            cx,
        );
    }

    /// The save finished. Success closes the dialog; a failure stays in it.
    pub fn saved(&mut self, result: Result<(), Failure>, cx: &mut Context<Self>) {
        self.busy = false;
        match result {
            Ok(()) => self.close = !self.inline,
            Err(failure) => self.error = Some(failure),
        }
        cx.notify();
    }

    pub fn open(dialog: Entity<Self>, window: &mut Window, cx: &mut App) {
        window.open_dialog(cx, move |view, _, cx| Self::frame(&dialog, view, cx));
    }

    fn frame(dialog: &Entity<Self>, view: Dialog, cx: &mut App) -> Dialog {
        let this = dialog.read(cx);
        let busy = this.busy;
        let ready = this.choice(cx).is_ok();
        let title = tr!("account-skin-title", name = this.name.as_str());
        let weak = dialog.downgrade();
        let save = {
            let weak = weak.clone();
            theme::clickable(
                Key::new("skin-save")
                    .label(tr!("account-skin-save"))
                    .primary()
                    .loading(busy)
                    .disabled(busy || !ready)
                    .debug_selector(|| "skin-save".into())
                    .on_click(move |_, window, cx| {
                        let _ = weak.update(cx, |dialog, cx| dialog.submit(window, cx));
                    }),
                !busy && ready,
            )
        };
        let cancel = theme::clickable(
            Key::new("skin-cancel")
                .label(tr!("common-cancel"))
                .white()
                .disabled(busy)
                .on_click(|_, window, cx| window.close_dialog(cx)),
            !busy,
        );
        theme::dialog(view, cx)
            .title(title)
            .w(px(DIALOG_WIDTH))
            .keyboard(!busy)
            .overlay_closable(!busy)
            .close_button(!busy)
            .child(dialog.clone())
            .footer(
                h_flex()
                    .w_full()
                    .justify_end()
                    .gap_2()
                    .child(cancel)
                    .child(save),
            )
    }

    /// A file picker for one of the picture fields.
    fn browse(field: &Entity<InputState>, id: &'static str) -> Key {
        let field = field.clone();
        kit::ghost(id, tr!("account-skin-browse"), move |window, cx| {
            let chosen =
                crate::platform::pick_path(cx, true, false, tr!("account-skin-pick-picture"));
            let (handle, field) = (window.window_handle(), field.clone());
            cx.spawn(async move |cx| {
                if let Some(path) = chosen.await {
                    let text = path.display().to_string();
                    let _ = cx.update_window(handle, |_, window, cx| {
                        field.update(cx, |field, cx| field.set_value(text, window, cx));
                    });
                }
            })
            .detach();
        })
        .debug_selector(move || id.into())
    }
}

fn label(text: &'static str, colors: ShellColors) -> impl IntoElement {
    div()
        .text_sm()
        .font_medium()
        .text_color(colors.foreground)
        .child(text)
}

impl Render for SkinDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.inline && std::mem::take(&mut self.close) && window.has_active_dialog(cx) {
            window.close_dialog(cx);
        }
        let colors = ShellColors::current(cx);
        let entity = cx.entity().downgrade();
        let busy = self.busy;
        let kinds = kinds().iter().enumerate().map(|(index, text)| {
            let entity = entity.clone();
            kit::led_option(
                ("skin-kind", index),
                *text,
                self.kind == index,
                colors,
                move |_, cx| {
                    let _ = entity.update(cx, |dialog, cx| {
                        if dialog.busy {
                            return;
                        }
                        dialog.kind = index;
                        dialog.error = None;
                        cx.notify();
                    });
                },
            )
            .debug_selector(move || format!("skin-kind-{index}"))
        });
        let detail = match self.kind {
            1 => v_flex()
                .gap_3()
                .child(
                    v_flex()
                        .gap_2()
                        .child(label(tr!("account-skin-model-label"), colors))
                        .child(kit::segments("skin-model", models(), self.model, {
                            let entity = entity.clone();
                            move |index, _, cx| {
                                let _ = entity.update(cx, |dialog, cx| {
                                    if dialog.busy {
                                        return;
                                    }
                                    dialog.model = index;
                                    cx.notify();
                                });
                            }
                        })),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(label(tr!("account-skin-skin-label"), colors))
                        .child(
                            h_flex()
                                .gap_2()
                                .child(div().flex_1().child(Input::new(&self.skin).disabled(busy)))
                                .child(Self::browse(&self.skin, "skin-browse").disabled(busy)),
                        ),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(label(tr!("account-skin-cape-label"), colors))
                        .child(
                            h_flex()
                                .gap_2()
                                .child(div().flex_1().child(Input::new(&self.cape).disabled(busy)))
                                .child(Self::browse(&self.cape, "cape-browse").disabled(busy)),
                        ),
                )
                .into_any_element(),
            2 => v_flex()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.muted)
                        .child(tr!("account-littleskin-hint")),
                )
                .child(h_flex().child(kit::ghost(
                    "skin-open-littleskin",
                    tr!("account-open-littleskin"),
                    |_, cx| {
                        crate::platform::open_address("https://littleskin.cn/", cx);
                    },
                )))
                .into_any_element(),
            3 => v_flex()
                .gap_2()
                .child(label(tr!("account-skin-api-label"), colors))
                .child(Input::new(&self.api).disabled(busy))
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.muted)
                        .child(tr!("account-skin-api-help")),
                )
                .into_any_element(),
            _ => div()
                .text_sm()
                .text_color(colors.muted)
                .child(tr!("account-skin-default-help"))
                .into_any_element(),
        };
        v_flex()
            .id("skin-dialog")
            .w_full()
            .gap_4()
            .child(v_flex().children(kinds))
            .child(detail)
            .children((self.kind != 0).then(|| {
                div()
                    .text_xs()
                    .text_color(colors.muted)
                    .child(tr!("account-skin-agent-note"))
            }))
            .children(self.error.clone().map(|(message, technical)| {
                h_flex()
                    .gap_2()
                    .items_center()
                    .debug_selector(|| "skin-error".into())
                    .child(div().text_sm().text_color(colors.danger).child(message))
                    .child(kit::technical("skin-technical", technical).xsmall())
            }))
            .when(self.inline, |form| {
                form.child(
                    Key::new("skin-inline-save")
                        .label(tr!("account-skin-save"))
                        .primary()
                        .loading(busy)
                        .disabled(busy || self.choice(cx).is_err())
                        .debug_selector(|| "skin-inline-save".into())
                        .on_click(cx.listener(|form, _, window, cx| form.submit(window, cx))),
                )
            })
    }
}

#[cfg(test)]
mod tests;
