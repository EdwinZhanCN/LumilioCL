//! The shell's part of making games: what the pages ask about new games
//! goes to the feature, not the application. The feature itself is tested in
//! `features::new_game`.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gpui::TestAppContext;

use super::super::LauncherShell;
use crate::live::LiveIntent;

#[gpui::test]
fn new_game_answers_dropped_files_and_the_application_the_rest(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let reached: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
    let sink = reached.clone();
    let (shell, cx) = cx.add_window_view(move |_, cx| {
        LauncherShell::new(cx).with_live(Rc::new(move |intent, _, _| {
            sink.borrow_mut().push(intent);
        }))
    });
    shell.update_in(cx, |shell, window, cx| {
        let handler = shell.live_handler.clone().unwrap();
        handler(
            LiveIntent::DropFiles(vec![PathBuf::from("notes.txt")]),
            window,
            cx,
        );
        handler(LiveIntent::ApplyUpdate, window, cx);
    });
    cx.run_until_parked();
    assert_eq!(reached.borrow().as_slice(), [LiveIntent::ApplyUpdate]);
    shell.read_with(cx, |shell, cx| {
        assert_eq!(
            shell.pending_toasts(cx).len(),
            1,
            "New Game said the file cannot be used"
        );
    });
}
