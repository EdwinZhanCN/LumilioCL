use super::super::intent::InstanceIntent;
use super::super::panels::{Arrived, Confirm, SHOTS_PAGE, Thumb};
use super::super::{InstanceDetailView, Section, TAB_SCREENSHOTS};
use super::{click, record, rooted};
use gpui::TestAppContext;
use lumilio_core::{LauncherSettings, ScreenshotInfo};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

fn shot(file: &str, modified_ms: i64) -> ScreenshotInfo {
    ScreenshotInfo {
        file: file.into(),
        path: PathBuf::from(format!("/game/screenshots/{file}")),
        modified_ms,
        size: 10,
    }
}

fn thumbnails(seen: &Rc<RefCell<Vec<InstanceIntent>>>) -> Vec<String> {
    seen.borrow()
        .iter()
        .filter_map(|intent| match intent {
            InstanceIntent::Thumbnail(file) => Some(file.clone()),
            _ => None,
        })
        .collect()
}

fn open(
    seen: Rc<RefCell<Vec<InstanceIntent>>>,
    shots: Vec<ScreenshotInfo>,
    cx: &mut TestAppContext,
) -> (
    gpui::Entity<InstanceDetailView>,
    &mut gpui::VisualTestContext,
) {
    let (view, cx) = rooted(cx, seen);
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_SCREENSHOTS, cx);
        view.arrived(Arrived::Screenshots(Ok(shots)), cx);
    });
    cx.run_until_parked();
    (view, cx)
}

#[gpui::test]
fn only_the_first_page_of_thumbnails_is_asked_for_and_more_on_request(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let shots: Vec<_> = (0..SHOTS_PAGE + 5)
        .map(|n| shot(&format!("{n}.png"), 1000 - n as i64))
        .collect();
    let (view, cx) = open(seen.clone(), shots, cx);
    assert_eq!(thumbnails(&seen).len(), SHOTS_PAGE);
    assert!(cx.debug_bounds("shot-0").is_some());
    assert_eq!(SHOTS_PAGE, 48, "the card ids below follow the page size");
    assert!(cx.debug_bounds("shot-47").is_some());
    assert!(cx.debug_bounds("shot-48").is_none());

    // The button is below the fold of the test window: the page scrolls.
    assert!(cx.debug_bounds("shots-more").is_some());
    view.update(cx, |view, cx| view.show_more_screenshots(cx));
    cx.run_until_parked();
    assert_eq!(
        thumbnails(&seen).len(),
        SHOTS_PAGE + 5,
        "the rest, each once"
    );
    assert!(
        cx.debug_bounds("shots-more").is_none(),
        "nothing more to show"
    );
    let _ = view;
}

#[gpui::test]
fn a_thumbnail_shows_when_it_arrives_and_a_changed_picture_is_asked_again(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = open(seen.clone(), vec![shot("a.png", 1), shot("b.png", 2)], cx);
    view.update(cx, |view, cx| {
        view.thumbnail_arrived("a.png".into(), Ok(PathBuf::from("/t/a.png")), cx);
        view.thumbnail_arrived("b.png".into(), Err("broken".into()), cx);
        // An answer nobody asked for is ignored.
        view.thumbnail_arrived("zzz.png".into(), Ok(PathBuf::from("/t/z.png")), cx);
    });
    view.read_with(cx, |view, _| {
        assert_eq!(view.thumbs["a.png"], Thumb::Ready(1, "/t/a.png".into()));
        assert_eq!(view.thumbs["b.png"], Thumb::Failed(2));
        assert!(!view.thumbs.contains_key("zzz.png"));
    });

    // a.png was replaced, b.png is unchanged, c.png is new: only those two are asked.
    let before = thumbnails(&seen).len();
    view.update(cx, |view, cx| {
        view.arrived(
            Arrived::Screenshots(Ok(vec![
                shot("a.png", 5),
                shot("b.png", 2),
                shot("c.png", 6),
            ])),
            cx,
        );
    });
    cx.run_until_parked();
    assert_eq!(thumbnails(&seen)[before..], ["a.png", "c.png"]);
}

#[gpui::test]
fn the_tab_loads_once_and_an_empty_wall_says_how_to_fill_it(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
    });
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.open_tab(TAB_SCREENSHOTS, window, cx);
            view.open_tab(TAB_SCREENSHOTS, window, cx);
        })
    });
    let loads = seen
        .borrow()
        .iter()
        .filter(|intent| **intent == InstanceIntent::Load(Section::Screenshots))
        .count();
    assert_eq!(loads, 1);
    view.update(cx, |view, cx| {
        view.arrived(Arrived::Screenshots(Ok(vec![])), cx)
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("shots-folder").is_some());
    assert!(cx.debug_bounds("shots-wall").is_none());
}

#[gpui::test]
fn the_viewer_steps_around_and_delete_asks_before_it_sends(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = open(
        seen.clone(),
        vec![shot("a.png", 3), shot("b.png", 2), shot("c.png", 1)],
        cx,
    );
    click(cx, "shot-1");
    view.read_with(cx, |view, _| {
        assert_eq!(view.shot_open.as_deref(), Some("b.png"))
    });
    assert!(cx.debug_bounds("shot-viewer").is_some());
    click(cx, "shot-next");
    view.read_with(cx, |view, _| {
        assert_eq!(view.shot_open.as_deref(), Some("c.png"))
    });
    click(cx, "shot-next");
    view.read_with(cx, |view, _| {
        assert_eq!(view.shot_open.as_deref(), Some("a.png"), "wraps around");
    });

    click(cx, "shot-copy");
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::CopyScreenshot("a.png".into()))
    );
    click(cx, "shot-reveal");
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::RevealPath("screenshots/a.png".into()))
    );

    let before = seen.borrow().len();
    click(cx, "shot-delete");
    assert_eq!(seen.borrow().len(), before, "deleting only asks");
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.confirm,
            Some(Confirm::DeleteScreenshot("a.png".into()))
        );
        assert_eq!(view.shot_open, None);
    });
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.confirmed(&Confirm::DeleteScreenshot("a.png".into()), window, cx)
        })
    });
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::DeleteScreenshot("a.png".into()))
    );
}
