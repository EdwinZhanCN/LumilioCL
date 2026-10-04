//! The warning shown the first time someone leaves out photosensitivity
//! triggers in Discover. After Modrinth App's `PhotosensitivityWarningModal`
//! (GPL-3.0-only; ADR 0022): the filter depends on what authors disclose, so
//! it is a help, not a guarantee.

use crate::key::Key;
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
            .title("关于光敏性筛选")
            .w(px(440.))
            .child(div().text_sm().text_color(muted).child(
                "这个筛选只会排除作者自己声明了有闪烁、频闪等光敏性内容的项目。\\
                         没有声明的项目不会被排除，所以它不能保证内容对光敏性癫痫患者是安全的。",
            ))
            .footer(
                h_flex()
                    .w_full()
                    .justify_end()
                    .gap_2()
                    .child(
                        Key::new("photosensitivity-forever")
                            .label("知道了，不再提示")
                            .white()
                            .on_click(move |_, window, cx| {
                                forever(window, cx);
                                window.close_dialog(cx);
                            }),
                    )
                    .child(
                        Key::new("photosensitivity-ok")
                            .label("知道了")
                            .primary()
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
            )
    });
}
