use super::*;
use gpui::{Modifiers, TestAppContext, point};
use worker::Mailbox;

#[test]
fn retina_frames_use_physical_pixels_and_large_frames_keep_their_aspect() {
    assert_eq!(physical_size(640., 360., 2.), Some((1280, 720)));
    assert_eq!(physical_size(8192., 4096., 2.), Some((4096, 2048)));
    assert_eq!(physical_size(0., 360., 2.), None);
}

struct ScrollPage {
    model: gpui::Entity<ModelView>,
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
        model: cx.new(|cx| ModelView::new(Rc::new(|_, _, _| {}), cx)),
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
        model.frame_arrived(Ok((
            Request {
                view: View::new(),
                width: 1,
                height: 1,
            },
            lumilio_schematic_render::Frame {
                width: 1,
                height: 1,
                bgra: vec![0, 0, 255, 255],
            },
        )));
        assert_eq!(model.state, State::Ready);
        assert!(model.image.is_some());
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("model-loading").is_none());
    assert!(cx.debug_bounds("model-error").is_none());
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
fn pointer_keyboard_and_reset_operate_the_inline_camera(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let (view, cx) = cx.add_window_view(|_, cx| ModelView::new(Rc::new(|_, _, _| {}), cx));
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
