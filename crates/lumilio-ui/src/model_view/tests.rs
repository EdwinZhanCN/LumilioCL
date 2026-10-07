use super::*;
use gpui::{Modifiers, TestAppContext, point};
use gpui_component::WindowExt as _;
use worker::Mailbox;

#[test]
fn retina_frames_use_physical_pixels_and_large_frames_keep_their_aspect() {
    assert_eq!(physical_size(640., 360., 2.), Some((1280, 720)));
    assert_eq!(physical_size(8192., 4096., 2.), Some((4096, 2048)));
    assert_eq!(physical_size(0., 360., 2.), None);
}

struct ScrollPage {
    model: gpui::Entity<Viewer>,
    scroll: gpui::ScrollHandle,
}

impl Render for ScrollPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("test-page")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(self.model.clone())
            .child(div().h(px(2000.)))
    }
}

#[gpui::test]
fn wheel_zoom_does_not_scroll_the_page_and_frames_replace_loading(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (page, cx) = cx.add_window_view(|_, cx| ScrollPage {
        model: cx.new(|cx| Viewer::new(Rc::new(|_, _, _| {}), cx)),
        scroll: gpui::ScrollHandle::new(),
    });
    cx.run_until_parked();
    let model = page.read_with(cx, |page, _| page.model.clone());
    let viewport = cx.debug_bounds("model-viewport").unwrap();
    cx.simulate_event(gpui::ScrollWheelEvent {
        position: viewport.center(),
        delta: ScrollDelta::Lines(point(0., -3.)),
        ..Default::default()
    });
    cx.run_until_parked();
    model.read_with(cx, |model, _| assert!(model.view.zoom < 1.));
    page.read_with(cx, |page, _| assert_eq!(page.scroll.offset().y, px(0.)));
    model.update(cx, |model, cx| {
        model.frame_arrived(Ok(Event::Loaded {
            undrawable: Vec::new(),
            explore_position: [0., 1.62, 3.],
        }));
        model.frame_arrived(Ok(one_pixel_frame()));
        assert_eq!(model.state, State::Ready);
        assert!(model.image.is_some());
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("model-loading").is_none());
    assert!(cx.debug_bounds("model-error").is_none());
    assert!(cx.debug_bounds("model-undrawable").is_none());
}

fn one_pixel_frame() -> Event {
    Event::Frame(lumilio_schematic_render::Frame {
        width: 1,
        height: 1,
        bgra: vec![0, 0, 255, 255],
    })
}

#[test]
fn undrawable_blocks_get_one_sentence_and_their_ids_in_technical_details() {
    assert_eq!(undrawable_note(&[]), None);
    let ids = ["minecraft:chain".to_string(), "mod:gadget".to_string()];
    let (sentence, details) = undrawable_note(&ids).unwrap();
    assert_eq!(sentence, "有 2 种方块这个游戏版本画不出来，预览里没有它们");
    assert!(!sentence.contains("minecraft:"));
    assert_eq!(details, "minecraft:chain\nmod:gadget");
}

#[gpui::test]
fn the_undrawable_note_shows_with_the_picture(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (model, cx) = cx.add_window_view(|_, cx| Viewer::new(Rc::new(|_, _, _| {}), cx));
    model.update(cx, |model, cx| {
        model.frame_arrived(Ok(Event::Loaded {
            undrawable: vec!["minecraft:chain".into()],
            explore_position: [0., 1.62, 3.],
        }));
        cx.notify();
    });
    cx.run_until_parked();
    // Nothing to qualify until the picture is there.
    assert!(cx.debug_bounds("model-undrawable").is_none());
    model.update(cx, |model, cx| {
        model.frame_arrived(Ok(one_pixel_frame()));
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("model-undrawable").is_some());
}

#[test]
fn requests_coalesce_and_closing_discards_pending_work() {
    let mailbox = Mailbox::default();
    for width in 1..=1000 {
        mailbox.put(Request {
            view: View::new(),
            width,
            height: 48,
        });
    }
    assert_eq!(mailbox.take().unwrap().width, 1000);
    mailbox.put(Request {
        view: View::new(),
        width: 32,
        height: 48,
    });
    mailbox.close();
    assert!(mailbox.take().is_none());
}

#[test]
fn closing_wakes_an_idle_worker() {
    let mailbox = Arc::new(Mailbox::default());
    let waiting = mailbox.clone();
    let (sent, received) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || sent.send(waiting.take()).unwrap());
    mailbox.close();
    assert!(
        received
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap()
            .is_none()
    );
    worker.join().unwrap();
}

#[test]
fn no_gpu_has_plain_copy_without_raw_technical_details() {
    assert_eq!(
        State::failed(SceneError::NoGpu),
        State::Failed {
            message: "这台电脑没有可用的图形设备，无法显示 3D 预览",
            detail: None,
        }
    );
    for error in [
        SceneError::Parse("invalid nbt".into()),
        SceneError::Pack("invalid zip".into()),
        SceneError::Mesh("bad model".into()),
        SceneError::Render("device lost".into()),
    ] {
        let State::Failed { message, detail } = State::failed(error) else {
            panic!()
        };
        assert!(!message.contains("invalid") && !message.contains("device lost"));
        assert!(detail.is_some());
    }
}

#[gpui::test]
fn pointer_keyboard_and_reset_operate_the_orbital_camera(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let (view, cx) = cx.add_window_view(|_, cx| Viewer::new(Rc::new(|_, _, _| {}), cx));
    cx.run_until_parked();
    let bounds = cx.debug_bounds("model-viewport").unwrap();
    let center = bounds.center();
    cx.simulate_click(center, Modifiers::none());
    cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(center, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_move(
        center + point(px(25.), px(10.)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    view.update(cx, |view, _| {
        assert_eq!(view.view.yaw_deg, 30.);
        assert_eq!(view.view.pitch_deg, 34.);
    });
    cx.simulate_mouse_move(center, None, Modifiers::none());
    cx.simulate_keystrokes("right");
    view.update(cx, |view, _| assert_eq!(view.view.yaw_deg, 20.));
    cx.simulate_keystrokes("r");
    view.update(cx, |view, _| {
        assert_eq!(view.view.yaw_deg, View::new().yaw_deg);
        assert_eq!(view.view.zoom, 1.);
    });
    view.update(cx, |view, cx| {
        view.state = State::failed(SceneError::NoGpu);
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("model-error").is_some());
}

#[test]
fn flight_is_time_based_and_releasing_input_stops_movement() {
    use std::time::Duration;
    let view = View {
        yaw_deg: 0.,
        position: Some([0.; 3]),
        ..View::new()
    };
    let mut input = FlightInput::default();
    assert!(input.key("w", true));
    let full = input.advance(view, Duration::from_secs(1));
    let half = input.advance(view, Duration::from_millis(500));
    assert_eq!(input.advance(half, Duration::from_millis(500)), full);
    // Auto-repeat is idempotent, opposite keys cancel, key-up clears the held key.
    input.key("w", true);
    assert_eq!(input.advance(view, Duration::from_secs(1)), full);
    input.key("s", true);
    assert_eq!(input.advance(view, Duration::from_secs(1)), view);
    input.key("s", false);
    assert_eq!(input.advance(view, Duration::from_secs(1)), full);
    input.key("space", true);
    input.shift = true;
    assert_eq!(input.advance(view, Duration::from_secs(1)), full);
    input.clear();
    assert_eq!(input.advance(full, Duration::from_secs(1)), full);
}

#[gpui::test]
fn modes_preserve_independent_cameras_and_release_held_keys(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (view, cx) = cx.add_window_view(|_, cx| Viewer::new(Rc::new(|_, _, _| {}), cx));
    cx.run_until_parked();
    let orbital = cx.debug_bounds("model-mode-key-0").unwrap().center();
    let explore = cx.debug_bounds("model-mode-key-1").unwrap().center();
    view.update(cx, |view, _| {
        view.view = view.view.orbit(25., 10.);
    });
    let orbit = view.read_with(cx, |view, _| view.view);
    cx.simulate_click(explore, Modifiers::none());
    let flight = view.read_with(cx, |view, _| {
        assert_eq!(view.mode, Mode::Explore);
        assert_eq!(view.view.position, Some([0., 1.62, 3.]));
        view.view
    });
    view.update(cx, |view, _| {
        view.view = view.view.moved(1., 0., 0., 5.);
        view.input.key("w", true);
    });
    cx.simulate_click(orbital, Modifiers::none());
    view.read_with(cx, |view, _| {
        assert_eq!(view.view, orbit);
        assert_eq!(
            view.input
                .advance(flight, std::time::Duration::from_secs(1)),
            flight
        );
    });
    cx.simulate_click(explore, Modifiers::none());
    view.read_with(cx, |view, _| {
        assert_eq!(view.view, flight.moved(1., 0., 0., 5.))
    });
}

#[gpui::test]
fn explore_before_loading_uses_the_scene_spawn_and_headless_capture_fails_visibly(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_component::init);
    let (viewer, cx) = cx.add_window_view(|_, cx| Viewer::new(Rc::new(|_, _, _| {}), cx));
    cx.run_until_parked();
    let explore = cx.debug_bounds("model-mode-key-1").unwrap().center();
    cx.simulate_click(explore, Modifiers::none());
    viewer.update(cx, |viewer, cx| {
        viewer.frame_arrived(Ok(Event::Loaded {
            undrawable: Vec::new(),
            explore_position: [50., 4., 75.],
        }));
        viewer.frame_arrived(Ok(one_pixel_frame()));
        cx.notify();
    });
    cx.run_until_parked();
    let viewport = cx.debug_bounds("model-viewport").unwrap().center();
    cx.simulate_click(viewport, Modifiers::none());
    viewer.read_with(cx, |viewer, _| {
        assert_eq!(viewer.view.position, Some([50., 4., 75.]));
        // Headless GPUI reports no native handle: never hide the real test runner's cursor.
        assert!(viewer.capture.is_none());
        assert!(viewer.capture_error.is_some());
        assert!(!viewer.clock_pending);
    });
}

#[gpui::test]
fn opening_is_lazy_escape_closes_and_reopening_uses_a_new_asset_request(cx: &mut TestAppContext) {
    use gpui_component::Root;
    use std::cell::RefCell;
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let requests: Rc<RefCell<Vec<u64>>> = Rc::default();
    let keep = requests.clone();
    let owner: Rc<RefCell<Option<gpui::Entity<ModelView>>>> = Rc::default();
    let store = owner.clone();
    let (_, cx) = cx.add_window_view(|window, cx| {
        let model =
            cx.new(|cx| ModelView::new(Rc::new(move |id, _, _| keep.borrow_mut().push(id)), cx));
        *store.borrow_mut() = Some(model.clone());
        Root::new(model, window, cx)
    });
    let owner = owner.borrow().clone().unwrap();
    cx.run_until_parked();
    assert!(requests.borrow().is_empty());
    assert!(cx.debug_bounds("model-viewport").is_none());
    let trigger = cx.debug_bounds("model-open").unwrap().center();
    cx.simulate_click(trigger, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(requests.borrow().len(), 1);
    assert!(cx.debug_bounds("model-viewport").is_some());
    let first = requests.borrow()[0];
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(cx.debug_bounds("model-viewport").is_none());
    assert_eq!(owner.read_with(cx, |owner, _| owner.request_id()), None);
    let trigger = cx.debug_bounds("model-open").unwrap().center();
    cx.simulate_click(trigger, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(requests.borrow().len(), 2);
    assert_ne!(requests.borrow()[1], first);
    cx.update(|window, cx| window.close_dialog(cx));
    cx.run_until_parked();
    assert_eq!(owner.read_with(cx, |owner, _| owner.request_id()), None);
}
