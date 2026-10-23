//! Before the launcher can be used, the person agrees to its disclaimer and
//! the Minecraft EULA (plan terms-consent). The release notes of an update
//! wait until then, so the two dialogs never stack.

use super::Wiring;
use gpui_kit::AppContext as _;
use gpui_kit::{AnyWindowHandle, App, Window};
use lumilio_ui::terms::{self, Occasion};
use std::rc::Rc;

/// Asks for the terms when they are not agreed to yet, then shows what the
/// update brought. Quitting the dialog quits the launcher.
pub(super) fn ask(wiring: &Wiring, window: AnyWindowHandle, cx: &mut App) {
    let service = wiring.backend.service.clone();
    let pending = wiring
        .backend
        .spawn(async move { service.settings().await.terms_pending() });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Some(pending) = super::jobs::answer(&wiring, pending, cx).await else {
            return;
        };
        if !pending {
            cx.update(|cx| super::update::announce(&wiring, window, cx));
            return;
        }
        let agree = {
            let wiring = wiring.clone();
            Rc::new(move |_: &mut Window, cx: &mut App| accept(&wiring, window, cx))
        };
        let quit = Rc::new(|_: &mut Window, cx: &mut App| cx.quit());
        let _ = cx.update_window(window, |_, window, cx| {
            terms::show(Occasion::Agree { agree, quit }, window, cx);
        });
    })
    .detach();
}

/// Remembers the agreement, then goes on as an agreed start does. A failed
/// write only means the terms are asked again next time.
fn accept(wiring: &Wiring, window: AnyWindowHandle, cx: &mut App) {
    let service = wiring.backend.service.clone();
    let saved = wiring
        .backend
        .spawn(async move { service.accept_terms().await });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        if let Some(Err(error)) = super::jobs::answer(&wiring, saved, cx).await {
            log::warn!("could not remember that the terms were agreed to: {error}");
        }
        cx.update(|cx| super::update::announce(&wiring, window, cx));
    })
    .detach();
}
