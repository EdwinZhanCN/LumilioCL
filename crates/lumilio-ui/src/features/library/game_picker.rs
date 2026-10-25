//! Which of the games found in another launcher's folder to bring over
//!. It holds no business logic: the answer is the games ticked.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{App, Context, Entity, IntoElement, Render, Window, div, px};
use gpui_component::{WindowExt as _, h_flex, v_flex};
use lumilio_core::{FoundGame, GameOrigin};

use crate::controls::Checkbox;
use crate::key::Key;
use crate::live::loader_label;
use crate::theme;
use crate::theme::ShellColors;

const DIALOG_WIDTH: f32 = 440.;

pub type PickHandler = Rc<dyn Fn(Vec<FoundGame>, &mut Window, &mut App)>;

/// Where a game was found, in the person's language.
#[must_use]
pub fn origin_label(origin: GameOrigin) -> String {
    match origin {
        GameOrigin::MultiMcPrism => "MultiMC / Prism".to_owned(),
        GameOrigin::MinecraftFolder => crate::tr!("game-picker-origin-minecraft").to_owned(),
    }
}

/// The games ticked, in the order found.
#[must_use]
pub fn chosen(rows: &[(FoundGame, bool)]) -> Vec<FoundGame> {
    rows.iter()
        .filter(|(_, on)| *on)
        .map(|(game, _)| game.clone())
        .collect()
}

/// What a row says: the name's game, loader and where it was found.
#[must_use]
pub fn describe(game: &FoundGame) -> String {
    format!(
        "{} · {} {} · {}",
        game.name,
        game.game_version,
        loader_label(game.loader),
        origin_label(game.origin)
    )
}

pub struct GamePicker {
    pub(super) rows: Vec<(FoundGame, bool)>,
    handler: PickHandler,
}

impl GamePicker {
    /// The first game starts ticked.
    pub fn new(games: Vec<FoundGame>, handler: PickHandler) -> Self {
        let rows = games
            .into_iter()
            .enumerate()
            .map(|(index, game)| (game, index == 0))
            .collect();
        Self { rows, handler }
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = chosen(&self.rows);
        if !picked.is_empty() {
            (self.handler)(picked, window, cx);
            window.close_dialog(cx);
        }
    }

    pub fn open(form: Entity<Self>, window: &mut Window, cx: &mut App) {
        window.open_dialog(cx, move |dialog, _, cx| {
            let ready = !chosen(&form.read(cx).rows).is_empty();
            let weak = form.downgrade();
            let ok = theme::clickable(
                Key::new("game-picker-ok")
                    .label(crate::tr!("game-picker-import"))
                    .primary()
                    .disabled(!ready)
                    .debug_selector(|| "game-picker-ok".into())
                    .on_click(move |_, window, cx| {
                        let _ = weak.update(cx, |form, cx| form.submit(window, cx));
                    }),
                ready,
            );
            theme::dialog(dialog, cx)
                .title(crate::tr!("game-picker-title"))
                .w(px(DIALOG_WIDTH))
                .child(form.clone())
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Key::new("game-picker-cancel")
                                .label(crate::tr!("common-cancel"))
                                .white()
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(ok),
                )
        });
    }
}

impl Render for GamePicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = ShellColors::current(cx);
        let weak = cx.entity().downgrade();
        v_flex()
            .id("game-picker")
            .w_full()
            .max_h(px(320.))
            .overflow_y_scroll()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .text_color(colors.muted)
                    .child(crate::tr!("game-picker-body")),
            )
            .children(self.rows.iter().enumerate().map(|(index, (game, on))| {
                let weak = weak.clone();
                Checkbox::new(("game-picker-row", index))
                    .checked(*on)
                    .label(describe(game))
                    .on_click(move |_, _, cx| {
                        let _ = weak.update(cx, |picker, cx| {
                            if let Some(row) = picker.rows.get_mut(index) {
                                row.1 = !row.1;
                            }
                            cx.notify();
                        });
                    })
            }))
    }
}
