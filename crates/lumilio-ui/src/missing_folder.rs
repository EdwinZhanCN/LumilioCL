//! The question a launch asks when the game's folder is gone: it was moved
//! or deleted outside the launcher. Asked while the launch moment waits,
//! instead of starting the game as an empty one (plan game-folder-missing).

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{App, Window, div, px};
use gpui_component::{WindowExt as _, h_flex};

use crate::key::Key;
use crate::{theme, tr};

const DIALOG_WIDTH: f32 = 440.;

/// How the person answered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingFolderAnswer {
    /// Make a new, empty folder and start the game in it.
    StartEmpty,
    /// Open the game's page, where it can also be deleted.
    ViewGame,
}

pub type MissingFolderHandler = Rc<dyn Fn(MissingFolderAnswer, &mut Window, &mut App)>;

// ia[home]: 启动前的游戏目录确认 | 继续 / 开始游戏后的弹窗 | 当前游戏的目录被移走或删掉时问一次：「作为空游戏启动」（新建空目录后启动）/「查看游戏」（打开它的概览，可新建空目录或删除）/「取消」；不再悄悄启动一个空游戏 | game-folder-missing T4
pub fn ask(name: &str, handler: MissingFolderHandler, window: &mut Window, cx: &mut App) {
    let name = name.to_owned();
    window.open_dialog(cx, move |dialog, _, cx| {
        let answer = |choice: MissingFolderAnswer| {
            let handler = handler.clone();
            move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                window.close_dialog(cx);
                handler(choice, window, cx);
            }
        };
        theme::dialog(dialog, cx)
            .title(tr!("missing-folder-title", name = name.as_str()))
            .w(px(DIALOG_WIDTH))
            .child(div().text_sm().child(tr!("missing-folder-body")))
            .footer(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(
                        Key::new("missing-folder-cancel")
                            .label(tr!("common-cancel"))
                            .white()
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(div().flex_1())
                    .child(
                        Key::new("missing-folder-view")
                            .label(tr!("missing-folder-view"))
                            .white()
                            .debug_selector(|| "missing-folder-view".into())
                            .on_click(answer(MissingFolderAnswer::ViewGame)),
                    )
                    .child(theme::clickable(
                        Key::new("missing-folder-start")
                            .label(tr!("missing-folder-start"))
                            .primary()
                            .debug_selector(|| "missing-folder-start".into())
                            .on_click(answer(MissingFolderAnswer::StartEmpty)),
                        true,
                    )),
            )
    });
}
