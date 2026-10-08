//! The warning shown the first time someone leaves out photosensitivity
//! triggers in Discover. After Modrinth App's `PhotosensitivityWarningModal`
//! (GPL-3.0-only; ADR 0022): the filter depends on what authors disclose, so
//! it is a help, not a guarantee.

use crate::key::Key;
use crate::tr;
use gpui::{App, Window, div, prelude::*, px};
use gpui_component::{ActiveTheme as _, WindowExt as _, h_flex};

// ia[discover]: 光敏性提示 | 第一次选「高级排除 · 光敏性触发」时的弹窗 | 说明这个筛选只依据作者的声明，不能保证安全；可选「知道了，不再提示」
pub fn warn(
    window: &mut Window,
    cx: &mut App,
    dismiss_forever: impl Fn(&mut Window, &mut App) + 'static,
) {
    let dismiss_forever = std::rc::Rc::new(dismiss_forever);
    window.open_dialog(cx, move |dialog, _, cx| {
        let muted = cx.theme().muted_foreground;
        let forever = dismiss_forever.clone();
        crate::theme::dialog(dialog, cx)
            .title(tr!("photosensitivity-title"))
            .w(px(440.))
            .child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child(tr!("photosensitivity-body")),
            )
            .footer(
                h_flex()
                    .w_full()
                    .justify_end()
                    .gap_2()
                    .child(
                        Key::new("photosensitivity-forever")
                            .label(tr!("photosensitivity-dismiss"))
                            .white()
                            .on_click(move |_, window, cx| {
                                forever(window, cx);
                                window.close_dialog(cx);
                            }),
                    )
                    .child(
                        Key::new("photosensitivity-ok")
                            .label(tr!("photosensitivity-ok"))
                            .primary()
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
            )
    });
}
