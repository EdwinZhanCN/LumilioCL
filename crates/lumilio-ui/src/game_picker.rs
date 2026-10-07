//! Which of the games found in another launcher's folder to bring over
//! (ADR 0016). It holds no business logic: the answer is the games ticked.

use std::rc::Rc;

use crate::controls::Checkbox;
use crate::key::Key;
use gpui::{App, Context, Entity, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::{ActiveTheme as _, WindowExt as _, h_flex, v_flex};
use lumilio_core::FoundGame;

use crate::live::loader_label;
use crate::theme::{self, ShellColors};

const DIALOG_WIDTH: f32 = 440.;

pub type PickHandler = Rc<dyn Fn(Vec<FoundGame>, &mut Window, &mut App)>;

/// The games ticked, in the order found.
#[must_use]
pub fn chosen(rows: &[(FoundGame, bool)]) -> Vec<FoundGame> {
    rows.iter()
        .filter(|(_, on)| *on)
        .map(|(game, _)| game.clone())
        .collect()
}

/// What a row says: the name's game and loader.
#[must_use]
pub fn describe(game: &FoundGame) -> String {
    format!(
        "{} · {} {}",
        game.name,
        game.game_version,
        loader_label(game.loader)
    )
}

pub struct GamePicker {
    rows: Vec<(FoundGame, bool)>,
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
        let colors = ShellColors::from_theme(cx.theme());
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

#[cfg(test)]
mod tests {
    use super::*;
    use lumilio_core::Loader;

    fn game(name: &str, loader: Loader) -> FoundGame {
        FoundGame {
            name: name.into(),
            game_version: "1.21.1".into(),
            loader,
            loader_version: None,
            game_dir: "/x".into(),
            origin: "t",
        }
    }

    #[test]
    fn the_first_game_starts_ticked_and_the_ticked_ones_are_handed_over_in_order() {
        let picker = GamePicker::new(
            vec![
                game("a", Loader::Vanilla),
                game("b", Loader::Fabric),
                game("c", Loader::Forge),
            ],
            Rc::new(|_, _, _| {}),
        );
        let names = |rows: &[(FoundGame, bool)]| -> Vec<String> {
            chosen(rows).into_iter().map(|g| g.name).collect()
        };
        assert_eq!(names(&picker.rows), ["a"]);
        let mut rows = picker.rows;
        rows[2].1 = true;
        rows[0].1 = false;
        assert_eq!(names(&rows), ["c"]);
        rows[1].1 = true;
        assert_eq!(names(&rows), ["b", "c"]);
        assert_eq!(describe(&rows[1].0), "b · 1.21.1 Fabric");
    }
}
