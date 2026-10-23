//! The launcher's disclaimer and the Minecraft EULA, the same points as the
//! README's 免责声明 / Disclaimer. Before the launcher can be used, and again
//! when `lumilio_core::TERMS_VERSION` rises, they must be agreed to;
//! Settings › About shows them again to read (plan terms-consent).

use std::rc::Rc;

use crate::key::Key;
use crate::theme::{self, ShellColors};
use crate::tr;
use gpui::{AnyElement, App, Window, div, prelude::*, px};
use gpui_component::text::TextView;
use gpui_component::{WindowExt as _, h_flex, v_flex};

/// Mojang's Minecraft End User License Agreement.
pub const EULA_URL: &str = "https://www.minecraft.net/eula";

const DIALOG_WIDTH: f32 = 520.;

pub type TermsHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Why the terms are shown, which decides the footer.
#[derive(Clone)]
pub enum Occasion {
    /// Before the launcher can be used: agree or quit. Esc, the overlay and
    /// the close button do nothing, so the choice is made with a key.
    Agree {
        agree: TermsHandler,
        quit: TermsHandler,
    },
    /// Opened from Settings › About to read again.
    Read,
}

fn points() -> [&'static str; 5] {
    [
        tr!("terms-unofficial"),
        tr!("terms-no-hosting"),
        tr!("terms-genuine"),
        tr!("terms-third-party"),
        tr!("terms-no-warranty"),
    ]
}

/// Opens the terms. Agreeing closes the dialog after `agree` runs.
// ia[home]: 同意免责声明与 Minecraft EULA | 第一次打开（以及条款更新后）的弹窗 | 「同意并继续」记下同意后进入启动器，之后再显示更新内容；「退出」关闭启动器；Esc 与点外面不关闭；「阅读 Minecraft EULA」在浏览器打开 EULA | 设置 › 关于 可再看
pub fn show(occasion: Occasion, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _, cx| {
        let colors = ShellColors::current(cx);
        let agreeing = matches!(occasion, Occasion::Agree { .. });
        let body = v_flex()
            .w_full()
            .gap_3()
            .pb_2()
            .debug_selector(|| "terms-body".into())
            .children(
                points()
                    .into_iter()
                    .enumerate()
                    .map(|(index, point)| item(index, point, colors)),
            )
            .when(agreeing, |body| {
                body.child(
                    div()
                        .pt_2()
                        .text_sm()
                        .text_color(colors.foreground)
                        .debug_selector(|| "terms-agree-note".into())
                        .child(tr!("terms-agree-note")),
                )
            });
        let eula = Key::new("terms-eula")
            .label(tr!("terms-eula"))
            .white()
            .debug_selector(|| "terms-eula".into())
            .on_click(|_, _, cx| cx.open_url(EULA_URL));
        theme::dialog(dialog, cx)
            .title(tr!("terms-title"))
            .w(px(DIALOG_WIDTH))
            .keyboard(!agreeing)
            .overlay_closable(!agreeing)
            .close_button(!agreeing)
            .child(body)
            .footer(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(eula)
                    .child(div().flex_1())
                    .children(actions(&occasion)),
            )
    });
}

fn actions(occasion: &Occasion) -> Vec<AnyElement> {
    match occasion {
        Occasion::Agree { agree, quit } => {
            let (agree, quit) = (agree.clone(), quit.clone());
            vec![
                Key::new("terms-quit")
                    .label(tr!("terms-quit"))
                    .white()
                    .debug_selector(|| "terms-quit".into())
                    .on_click(move |_, window, cx| quit(window, cx))
                    .into_any_element(),
                Key::new("terms-agree")
                    .label(tr!("terms-agree"))
                    .primary()
                    .debug_selector(|| "terms-agree".into())
                    .on_click(move |_, window, cx| {
                        agree(window, cx);
                        window.close_dialog(cx);
                    })
                    .into_any_element(),
            ]
        }
        Occasion::Read => vec![
            Key::new("terms-done")
                .label(tr!("release-notes-done"))
                .primary()
                .debug_selector(|| "terms-done".into())
                .on_click(|_, window, cx| window.close_dialog(cx))
                .into_any_element(),
        ],
    }
}

fn item(index: usize, point: &'static str, colors: ShellColors) -> impl IntoElement {
    h_flex()
        .w_full()
        .items_start()
        .gap_3()
        .child(
            // The release notes' quiet square marker: it carries no state.
            div().flex_none().mt(px(8.)).size(px(4.)).bg(colors.muted),
        )
        .child(
            TextView::markdown(("terms-point", index), point)
                .flex_1()
                .min_w_0()
                .text_sm()
                .text_color(colors.foreground),
        )
}

#[cfg(test)]
mod tests;
