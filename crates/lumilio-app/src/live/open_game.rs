//! Opens a game's page from what the application does (a new game, the
//! review hooks). Deferred: these may run inside one of the shell's own
//! updates.

use gpui_kit::{App, Window};
use lumilio_ui::instance_detail::InstanceIntent;

use super::Wiring;

/// Opens a game's page; `then` is something to ask of the page once it is up.
pub(super) fn open_instance(
    wiring: &Wiring,
    id: String,
    then: Option<InstanceIntent>,
    window: &mut Window,
    cx: &mut App,
) {
    let shell = wiring.shell.clone();
    window.defer(cx, move |window, cx| {
        let _ = shell.update(cx, |shell, cx| shell.open_game(id, then, window, cx));
    });
}
