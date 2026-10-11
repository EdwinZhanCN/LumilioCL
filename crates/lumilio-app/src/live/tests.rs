use super::jobs::outcome;
use lumilio_ui::toast::Toast;

#[gpui_kit::test]
fn home_snapshot_and_rapid_selection_keep_the_last_game(cx: &mut gpui_kit::TestAppContext) {
    // The shipped backend wakes GPUI from its real tokio worker threads.
    cx.executor().allow_parking();
    use super::{Reload, State, Wiring, jobs::reload, library::choose_current};
    use crate::backend::Backend;
    use gpui_kit::AppContext as _;
    use lumilio_ui::{LauncherShell, home::HomePresentation};
    use std::{
        cell::RefCell,
        rc::Rc,
        time::{Duration, Instant},
    };

    let dir = tempfile::tempdir().unwrap();
    let backend = Backend::open(dir.path().join("root")).unwrap();
    let service = backend.service.clone();
    let (a, b) = futures::executor::block_on(backend.spawn(async move {
        let a = service
            .create_instance("A", Some("1.0"), lumilio_core::Loader::Vanilla, None)
            .await
            .unwrap();
        let b = service
            .create_instance("B", Some("1.0"), lumilio_core::Loader::Vanilla, None)
            .await
            .unwrap();
        service.set_current_instance(&b.id).await.unwrap();
        (a.id, b.id)
    }))
    .unwrap();
    cx.update(gpui_kit::init);
    let shell = cx.new(|cx| {
        LauncherShell::new(cx)
            .with_home(HomePresentation::Loading, cx)
            .with_live(Rc::new(|_, _, _| {}))
    });
    let wiring = Wiring {
        backend: Rc::new(backend),
        shell: shell.downgrade(),
        state: Rc::new(RefCell::new(State::default())),
    };
    cx.update(|cx| reload(&wiring, Reload::All, cx));
    let wait = |cx: &mut gpui_kit::TestAppContext, wanted: &str| {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            cx.run_until_parked();
            let ready = shell.read_with(cx, |shell, _| {
                matches!(shell.home(),
                HomePresentation::Continue { subject, .. } if subject.id.as_deref() == Some(wanted))
            });
            if ready {
                break;
            }
            assert!(Instant::now() < until, "Home did not show {wanted}");
            std::thread::sleep(Duration::from_millis(1));
        }
    };
    wait(cx, &b);
    cx.update(|cx| {
        reload(&wiring, Reload::All, cx);
        choose_current(&wiring, b.clone(), cx);
        choose_current(&wiring, a.clone(), cx);
    });
    wait(cx, &a);
    let service = wiring.backend.service.clone();
    assert_eq!(
        futures::executor::block_on(
            wiring
                .backend
                .spawn(async move { service.current_instance().await })
        )
        .unwrap()
        .as_deref(),
        Some(a.as_str())
    );
    cx.run_until_parked();
    assert_eq!(
        wiring.state.borrow().continue_id.as_deref(),
        Some(a.as_str())
    );
    let latest = shell.read_with(cx, |shell, _| shell.live().unwrap().install_target.clone());
    assert_eq!(latest.as_deref(), Some(a.as_str()));
}

#[test]
fn a_failed_operation_says_it_plainly_and_keeps_the_cause_behind_details() {
    let failed = outcome(
        Err::<String, _>("os error 28: no space left"),
        |file| format!("已装好 {file}"),
        "没有装上 Sodium".to_owned(),
    );
    assert_eq!(
        failed,
        Toast::error("没有装上 Sodium").technical("os error 28: no space left")
    );
    assert!(
        !failed.text.contains("os error"),
        "no raw error as headline"
    );
    let done = outcome(
        Ok::<_, String>("sodium.jar"),
        |file| format!("已装好 {file}"),
        String::new(),
    );
    assert_eq!(done, Toast::success("已装好 sodium.jar"));
}
