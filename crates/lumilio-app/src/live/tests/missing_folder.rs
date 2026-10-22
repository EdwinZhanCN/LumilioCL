//! Acceptance of plan game-folder-missing (T4): a game whose folder was
//! moved or deleted is asked about before anything starts, never quietly
//! started in an empty folder.

use super::super::context::follow;
use super::super::dispatch::on_shell_intent;
use super::global_context::{ask, continue_subject, failed_attempts, game, open, until};
use gpui_kit::component::WindowExt as _;
use gpui_kit::{Modifiers, TestAppContext};
use lumilio_ui::ShellIntent;
use lumilio_ui::home::HomeIntent;

#[gpui_kit::test]
fn a_game_whose_folder_is_gone_is_asked_about_before_it_starts(cx: &mut TestAppContext) {
    let (shell, wiring, cx) = open(cx);
    cx.update(|_, cx| cx.set_reduce_motion(true));
    // A version that does not exist: a launch fails while preparing.
    let a = game(&wiring, "A", "lumilio-test-none-a");
    ask(&wiring, {
        let a = a.clone();
        move |service| async move {
            service.add_account("Steve", None).await?;
            service.set_current_instance(&a).await
        }
    })
    .unwrap();
    let folder = wiring.backend.service.game_dir(&a);
    std::fs::remove_dir_all(&folder).unwrap();
    cx.update(|_, cx| follow(&wiring, cx));
    until(cx, "Home to continue A", |cx| {
        continue_subject(&shell, cx).as_deref() == Some(a.as_str())
    });

    cx.update(|window, cx| {
        on_shell_intent(&wiring, ShellIntent::Home(HomeIntent::Continue), window, cx)
    });
    until(cx, "the question about A's folder", |cx| {
        cx.update(|window, cx| window.has_active_dialog(cx))
    });
    assert_eq!(failed_attempts(&wiring, &a), 0, "nothing was started");
    assert!(!folder.exists());

    let bounds = cx
        .debug_bounds("missing-folder-start")
        .expect("作为空游戏启动 is on screen");
    cx.simulate_click(bounds.center(), Modifiers::none());
    until(cx, "A to start with an empty folder", |_| {
        failed_attempts(&wiring, &a) == 1
    });
    assert!(folder.is_dir());
}
