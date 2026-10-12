//! The one question a launch asks about Java: the Java chosen for this game
//! cannot run it. Asked once per launch, while the launch moment waits; the
//! answer decides this launch only and leaves the game's settings alone.

use std::rc::Rc;

use crate::key::Key;
use crate::theme;
use crate::tr;
use gpui::{App, Window, div, prelude::*, px};
use gpui_component::{WindowExt as _, h_flex};
use lumilio_core::InstanceJava;

const DIALOG_WIDTH: f32 = 440.;

/// What 本次改用 would start the game with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JavaSwitch {
    /// An installed Java of this major.
    Installed(u32),
    /// None fits; this major is downloaded first.
    Download(u32),
}

/// The chosen Java breaks a rule the game cannot start without.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JavaConflict {
    pub chosen: u32,
    pub needed: u32,
    /// `None` when the launch could not switch (nothing fits and downloads
    /// on launch are off).
    pub switch: Option<JavaSwitch>,
}

impl JavaConflict {
    /// From what the service says about a game's Java; `None` when the
    /// launch has nothing to ask.
    #[must_use]
    pub fn of(java: &InstanceJava) -> Option<Self> {
        let choice = java.choice.as_ref()?;
        if !choice.chosen_by_user || !choice.breaks_required() {
            return None;
        }
        let needed = choice.advised_major()?;
        let switch = match (&java.automatic, java.needed) {
            (Some(automatic), _) => Some(JavaSwitch::Installed(automatic.runtime.major())),
            (None, Some(major)) if java.auto_install => Some(JavaSwitch::Download(major)),
            _ => None,
        };
        Some(Self {
            chosen: choice.runtime.major(),
            needed,
            switch,
        })
    }
}

/// How the person answered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JavaConflictAnswer {
    /// Start with the automatic Java this once.
    Switch,
    /// Start with the chosen Java all the same.
    Anyway,
}

pub type JavaConflictHandler = Rc<dyn Fn(JavaConflictAnswer, &mut Window, &mut App)>;

// ia[home]: 启动前的 Java 确认 | 继续 / 开始游戏后的弹窗 | 游戏设置里选的 Java 不能运行这个版本时问一次：「本次改用 Java N」（本机没有合适的则「下载并使用 Java N」）/「仍然启动」/「取消」；只影响这一次，不改游戏设置 | java-management T6
pub fn ask(
    conflict: &JavaConflict,
    handler: JavaConflictHandler,
    window: &mut Window,
    cx: &mut App,
) {
    let conflict = conflict.clone();
    window.open_dialog(cx, move |dialog, _, cx| {
        let answer = |choice: JavaConflictAnswer| {
            let handler = handler.clone();
            move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                window.close_dialog(cx);
                handler(choice, window, cx);
            }
        };
        let switch = conflict.switch.map(|switch| {
            let label = match switch {
                JavaSwitch::Installed(major) => tr!("java-conflict-switch", major = major),
                JavaSwitch::Download(major) => tr!("java-conflict-download", major = major),
            };
            theme::clickable(
                Key::new("java-conflict-switch")
                    .label(label)
                    .primary()
                    .debug_selector(|| "java-conflict-switch".into())
                    .on_click(answer(JavaConflictAnswer::Switch)),
                true,
            )
        });
        let anyway = Key::new("java-conflict-anyway")
            .label(tr!("java-conflict-anyway"))
            .white()
            .debug_selector(|| "java-conflict-anyway".into())
            .on_click(answer(JavaConflictAnswer::Anyway));
        theme::dialog(dialog, cx)
            .title(tr!("java-conflict-title", needed = conflict.needed))
            .w(px(DIALOG_WIDTH))
            .child(
                div()
                    .text_sm()
                    .child(tr!("java-conflict-body", chosen = conflict.chosen)),
            )
            .footer(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(
                        Key::new("java-conflict-cancel")
                            .label(tr!("common-cancel"))
                            .white()
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(div().flex_1())
                    .child(anyway)
                    .children(switch),
            )
    });
}

#[cfg(test)]
mod tests;
