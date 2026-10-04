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
use gpui_component::{
    ActiveTheme as _, Sizable as _, StyledExt as _, WindowExt as _, h_flex, v_flex,
};
use lumilio_core::{SkinChoice, SkinModel};

use crate::kit;
use crate::new_game::Failure;
use crate::theme::{self, ShellColors};

const DIALOG_WIDTH: f32 = 480.;

pub const LITTLE_SKIN_HINT: &str = "你需要在 LittleSkin 上创建一个和这个离线账户同名的角色。之后账户的皮肤就是皮肤站上那个角色所设置的。";
pub const AGENT_NOTE: &str = "选了皮肤后，启动游戏时启动器会在本机起一个小小的皮肤服务器，并加载 authlib-injector（第一次会自动下载）。";
const KINDS: [&str; 4] = [
    "默认",
    "本地文件",
    "LittleSkin",
    "皮肤站（CustomSkinLoader）",
];
const MODELS: [&str; 2] = ["经典（宽臂）", "纤细（窄臂）"];

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
        let skin = input(skin, "皮肤图片（PNG）", cx);
        let cape = input(cape, "披风图片（PNG，可空）", cx);
        let api = input(api, "皮肤站地址（CustomSkinLoader API）", cx);
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
        }
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
                    return Err("选一个皮肤图片，或者一个披风图片");
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
                    Err("请输入皮肤站地址")
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
            Ok(()) => self.close = true,
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
        let title = format!("{} 的皮肤", this.name);
        let weak = dialog.downgrade();
        let save = {
            let weak = weak.clone();
            theme::clickable(
                Key::new("skin-save")
                    .label("保存")
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
                .label("取消")
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
    fn browse(field: &Entity<InputState>, id: &'static str) -> impl IntoElement {
        let field = field.clone();
        kit::ghost(id, "浏览…", move |window, cx| {
            let chosen = crate::platform::pick_path(cx, true, false, "选择图片");
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
        if std::mem::take(&mut self.close) && window.has_active_dialog(cx) {
            window.close_dialog(cx);
        }
        let colors = ShellColors::from_theme(cx.theme());
        let entity = cx.entity().downgrade();
        let busy = self.busy;
        let kinds = KINDS.iter().enumerate().map(|(index, text)| {
            let entity = entity.clone();
            kit::led_option(
                ("skin-kind", index),
                *text,
                self.kind == index,
                colors,
                move |_, cx| {
                    let _ = entity.update(cx, |dialog, cx| {
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
                    v_flex().gap_2().child(label("模型", colors)).child(kit::segments(
                        "skin-model",
                        &MODELS,
                        self.model,
                        {
                            let entity = entity.clone();
                            move |index, _, cx| {
                                let _ = entity.update(cx, |dialog, cx| {
                                    dialog.model = index;
                                    cx.notify();
                                });
                            }
                        },
                    )),
                )
                .child(
                    v_flex().gap_2().child(label("皮肤图片", colors)).child(
                        h_flex()
                            .gap_2()
                            .child(div().flex_1().child(Input::new(&self.skin).disabled(busy)))
                            .child(Self::browse(&self.skin, "skin-browse")),
                    ),
                )
                .child(
                    v_flex().gap_2().child(label("披风图片", colors)).child(
                        h_flex()
                            .gap_2()
                            .child(div().flex_1().child(Input::new(&self.cape).disabled(busy)))
                            .child(Self::browse(&self.cape, "cape-browse")),
                    ),
                )
                .into_any_element(),
            2 => v_flex()
                .gap_2()
                .child(div().text_sm().text_color(colors.muted).child(LITTLE_SKIN_HINT))
                .child(h_flex().child(kit::ghost("skin-open-littleskin", "打开 LittleSkin", |_, cx| {
                    crate::platform::open_address("https://littleskin.cn/", cx);
                })))
                .into_any_element(),
            3 => v_flex()
                .gap_2()
                .child(label("皮肤站地址", colors))
                .child(Input::new(&self.api).disabled(busy))
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.muted)
                        .child("地址下需要有 <玩家名>.json 和 textures/ 目录，LittleSkin 和 Blessing Skin 皮肤站都是这样。"),
                )
                .into_any_element(),
            _ => div()
                .text_sm()
                .text_color(colors.muted)
                .child("游戏按玩家 UUID 自己挑一个默认皮肤，启动时不需要额外的东西。")
                .into_any_element(),
        };
        v_flex()
            .id("skin-dialog")
            .w_full()
            .gap_4()
            .child(v_flex().children(kinds))
            .child(detail)
            .children(
                (self.kind != 0)
                    .then(|| div().text_xs().text_color(colors.muted).child(AGENT_NOTE)),
            )
            .children(self.error.clone().map(|(message, technical)| {
                h_flex()
                    .gap_2()
                    .items_center()
                    .debug_selector(|| "skin-error".into())
                    .child(div().text_sm().text_color(colors.danger).child(message))
                    .child(kit::technical("skin-technical", technical).xsmall())
            }))
    }
}

#[cfg(test)]
mod tests;
