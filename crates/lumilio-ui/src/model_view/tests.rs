use gpui::{Modifiers, TestAppContext, point};
use gpui_component::WindowExt as _;
use worker::Mailbox;

use super::*;

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
            undrawable_entities: Vec::new(),
            explore_position: [0., 1.62, 3.],
            cells: ([0; 3], [0; 3]),
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
    Event::Frame {
        frame: lumilio_schematic_render::Frame {
            width: 1,
            height: 1,
            bgra: vec![0, 0, 255, 255],
        },
        mark: None,
    }
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
            undrawable_entities: vec!["minecraft:fox".into()],
            explore_position: [0., 1.62, 3.],
            cells: ([0; 3], [0; 3]),
        }));
        cx.notify();
    });
    cx.run_until_parked();
    // Nothing to qualify until the picture is there.
    assert!(cx.debug_bounds("model-undrawable").is_none());
    assert!(cx.debug_bounds("model-undrawable-entities").is_none());
    model.update(cx, |model, cx| {
        model.frame_arrived(Ok(one_pixel_frame()));
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("model-undrawable").is_some());
    assert!(cx.debug_bounds("model-undrawable-entities").is_some());
}

#[test]
fn undrawable_entities_get_their_own_sentence_and_ids_in_technical_details() {
    assert_eq!(undrawable_entities_note(&[]), None);
    let ids = ["minecraft:fox".to_string(), "minecraft:axolotl".to_string()];
    let (sentence, details) = undrawable_entities_note(&ids).unwrap();
    assert_eq!(sentence, "有 2 种实体这个游戏版本画不出来，预览里没有它们");
    assert_eq!(details, "minecraft:fox\nminecraft:axolotl");
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
    assert!(matches!(mailbox.take(), Some(worker::Work::Frame(request)) if request.width == 1000));
    mailbox.put(Request {
        view: View::new(),
        width: 32,
        height: 48,
    });
    mailbox.close();
    assert!(mailbox.take().is_none());
}

/// Picks and exports go before the next viewport frame, and closing the
/// mailbox (dropping the viewport) ends whoever waits on one (T10).
#[test]
fn jobs_go_first_and_closing_ends_their_replies() {
    let mailbox = Mailbox::default();
    let request = Request {
        view: View::new(),
        width: 8,
        height: 8,
    };
    mailbox.put(request);
    let (reply, picked) = async_channel::bounded(1);
    mailbox.queue(Job::Pick {
        request,
        x: 1.,
        y: 1.,
        reply,
    });
    assert!(matches!(
        mailbox.take(),
        Some(worker::Work::Job(Job::Pick { .. }))
    ));
    assert!(matches!(mailbox.take(), Some(worker::Work::Frame(_))));
    let (reply, exported) = async_channel::bounded(1);
    mailbox.queue(Job::Export { request, reply });
    mailbox.close();
    assert!(picked.recv_blocking().is_err());
    assert!(
        exported.recv_blocking().is_err(),
        "the export waits no more"
    );
    assert!(mailbox.take().is_none());
}

#[gpui::test]
fn an_angle_preset_turns_the_orbit_camera_and_leaves_explore(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (viewer, cx) =
        cx.add_window_view(|_, cx| Viewer::inline(Rc::new(|_, _, _| {}), Rc::new(|_| {}), cx));
    viewer.update(cx, |viewer, cx| {
        viewer.select_mode(Mode::Explore, cx);
        viewer.turn_to(Angle::North, cx);
        assert_eq!(viewer.mode, Mode::Orbital);
        assert_eq!((viewer.view.yaw_deg, viewer.view.pitch_deg), (180., 0.));
        assert_eq!(viewer.view.position, None);
        viewer.turn_to(Angle::Top, cx);
        assert_eq!(viewer.view.pitch_deg, View::MAX_PITCH);
    });
}

/// T32: a click inspects one block, which the worker is told to outline in
/// the theme's ink; an empty click or Esc lets it go. Hovering is told too.
#[gpui::test]
fn an_inspected_block_is_outlined_until_let_go(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let told: Rc<std::cell::Cell<usize>> = Rc::default();
    let seen = told.clone();
    let (viewer, cx) = cx.add_window_view(move |_, cx| {
        Viewer::inline(
            Rc::new(|_, _, _| {}),
            Rc::new(move |_| seen.set(seen.get() + 1)),
            cx,
        )
    });
    let mailbox = viewer.update(cx, |viewer, _| viewer.start_idle_worker());
    let stone = lumilio_schematic_render::Picked {
        position: [1, 2, 3],
        state: "minecraft:stone".into(),
    };
    let marks = |mailbox: &Mailbox| {
        std::iter::from_fn(|| mailbox.try_take())
            .filter_map(|work| match work {
                Work::Job(Job::Mark { position, .. }) => Some(position),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    viewer.update(cx, |viewer, cx| {
        viewer.inspected_arrived(
            Some(super::Inspected {
                picked: stone.clone(),
                contents: None,
            }),
            cx,
        );
        assert_eq!(
            viewer.inspected().map(|inspected| &inspected.picked),
            Some(&stone)
        );
    });
    assert_eq!(marks(&mailbox), [Some([1, 2, 3])], "outlined");
    cx.run_until_parked();
    assert_eq!(marks(&mailbox), [], "a draw sends it only on a change");

    viewer.update(cx, |viewer, cx| viewer.inspected_arrived(None, cx));
    assert_eq!(marks(&mailbox), [None], "an empty click lets it go");
    viewer.update(cx, |viewer, cx| {
        viewer.inspected_arrived(
            Some(super::Inspected {
                picked: stone.clone(),
                contents: None,
            }),
            cx,
        );
        viewer.let_go(cx);
        assert!(viewer.inspected().is_none());
    });
    assert_eq!(marks(&mailbox), [Some([1, 2, 3]), None]);

    viewer.update(cx, |viewer, cx| {
        viewer.hovered_for_test(Some(stone.clone()), cx);
        assert_eq!(viewer.pointed(), Some(&stone), "the read-out names it");
        viewer.leave(cx);
        assert_eq!(viewer.pointed(), None, "off the picture, the hint again");
    });
    assert_eq!(told.get(), 6, "each change is told to the studio");
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
fn no_block_assets_says_why_in_one_calm_sentence(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (model, cx) = cx.add_window_view(|_, cx| Viewer::new(Rc::new(|_, _, _| {}), cx));
    let without = |notice| {
        Ok(ModelPreview {
            schematic: Vec::new(),
            pack: None,
            plan: lumilio_core::AssetPlan {
                sources: Vec::new(),
                notice,
                ..lumilio_core::AssetPlan::default()
            },
            entities: Default::default(),
        })
    };
    for (notice, sentence) in [
        (Some(AssetNotice::Offline), "没有网络，方块资源还没下载过"),
        (
            Some(AssetNotice::Unavailable),
            "方块资源服务器上还没有这份资源（网络没有问题）",
        ),
        (
            Some(AssetNotice::DownloadDisabled),
            "自动下载方块资源已关闭，方块资源还没下载过",
        ),
    ] {
        model.update(cx, |model, cx| {
            model.assets(without(notice), cx);
            assert_eq!(
                model.state,
                State::Failed {
                    message: sentence,
                    detail: None,
                }
            );
        });
    }
    cx.run_until_parked();
    assert!(cx.debug_bounds("model-error").is_some());
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
            undrawable_entities: Vec::new(),
            explore_position: [50., 4., 75.],
            cells: ([0; 3], [0; 3]),
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
    use std::cell::RefCell;

    use gpui_component::Root;
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

/// A loaded, drawn inline viewer on an idle worker, with these cells.
fn loaded_inline(
    cx: &mut TestAppContext,
    cells: ([i32; 3], [i32; 3]),
) -> (
    gpui::Entity<Viewer>,
    Arc<Mailbox>,
    &mut gpui::VisualTestContext,
) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let (viewer, cx) =
        cx.add_window_view(|_, cx| Viewer::inline(Rc::new(|_, _, _| {}), Rc::new(|_| {}), cx));
    let mailbox = viewer.update(cx, |viewer, _| viewer.start_idle_worker());
    viewer.update(cx, |viewer, cx| {
        viewer.frame_arrived(Ok(Event::Loaded {
            undrawable: Vec::new(),
            undrawable_entities: Vec::new(),
            explore_position: [0., 1.62, 3.],
            cells,
        }));
        viewer.frame_arrived(Ok(one_pixel_frame()));
        cx.notify();
    });
    cx.run_until_parked();
    (viewer, mailbox, cx)
}

/// A picture this wide holding a loaded, drawn inline viewer.
struct Picture(gpui::Entity<Viewer>, f32);

impl Render for Picture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(self.1)).h(px(400.)).flex().child(self.0.clone())
    }
}

/// Chooses 切片's `choice` (0 不切片, then X, Y, Z) in its dropdown, as a
/// person's click does: the dropdown confirms it.
fn choose_in_dropdown(
    viewer: &gpui::Entity<Viewer>,
    choice: usize,
    cx: &mut gpui::VisualTestContext,
) {
    use gpui_component::IndexPath;
    use gpui_component::select::SelectEvent;
    let pick = viewer
        .read_with(cx, |viewer, _| viewer.slice_pick_for_test())
        .expect("切片's dropdown is made on the first draw");
    cx.update(|window, cx| {
        pick.update(cx, |pick, cx| {
            pick.set_selected_index(Some(IndexPath::new(choice)), window, cx);
            let value = pick.selected_value().cloned();
            cx.emit(SelectEvent::Confirm(value));
        })
    });
    cx.run_until_parked();
}

/// What the layer field shows.
fn layer_text(viewer: &gpui::Entity<Viewer>, cx: &mut gpui::VisualTestContext) -> String {
    let field = viewer
        .read_with(cx, |viewer, _| viewer.layer_field_for_test())
        .expect("the layer field");
    field.read_with(cx, |field, _| field.value().to_string())
}

/// The studio's tools are two plates on the picture's bottom corners. The
/// leading one holds the camera's keys: Orbital / Explore and 角度 (复位视角
/// first, then the presets). The trailing one holds 切片, a labelled
/// dropdown as the discover page's 排序方式, and while slicing the layer
/// field over it. Every control stands on one line across both. At the
/// narrowest picture (the 720 px window's, about 400 px) the plates stay
/// apart and inside it: one strip of every key wrapped over the read-out
/// there.
#[gpui::test]
fn the_tool_plates_keep_their_controls_on_one_line_and_apart(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let width = 396.;
    let (picture, cx) = cx.add_window_view(|_, cx| {
        let viewer = cx.new(|cx| Viewer::inline(Rc::new(|_, _, _| {}), Rc::new(|_| {}), cx));
        Picture(viewer, width)
    });
    let viewer = picture.read_with(cx, |picture, _| picture.0.clone());
    let _mailbox = viewer.update(cx, |viewer, _| viewer.start_idle_worker());
    viewer.update(cx, |viewer, cx| {
        viewer.frame_arrived(Ok(Event::Loaded {
            undrawable: Vec::new(),
            undrawable_entities: Vec::new(),
            explore_position: [0., 1.62, 3.],
            cells: ([0, 0, 0], [3, 5, 2]),
        }));
        viewer.frame_arrived(Ok(one_pixel_frame()));
        cx.notify();
    });
    cx.run_until_parked();
    let bounds = |cx: &mut gpui::VisualTestContext, selector: &'static str| {
        cx.debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} is drawn"))
    };

    let angles = viewer.update(cx, |viewer, cx| viewer.angle_entries(cx));
    assert_eq!(angles[0].label().as_ref(), "复位视角（R）");
    assert_eq!(angles.len(), 7, "复位视角, then the six presets");
    assert!(
        cx.debug_bounds("model-slice-layer").is_none(),
        "no layer field until slicing"
    );
    choose_in_dropdown(&viewer, 2, cx);
    assert_eq!(
        viewer.read_with(cx, |viewer, _| viewer.slice().map(|slice| slice.layer)),
        Some(5),
        "Y 轴 slices from the top"
    );
    assert_eq!(layer_text(&viewer, cx), "5");

    let line = bounds(cx, "model-mode-key-1").bottom();
    for control in ["model-mode-key-0", "model-angle", "model-slice"] {
        let control_bounds = bounds(cx, control);
        let off = (control_bounds.bottom() - line).abs();
        assert!(
            off <= theme::KEY_TRAVEL,
            "{control} is {off:?} off the line"
        );
        assert_eq!(
            control_bounds.size.height,
            px(KeySize::Small.height()),
            "{control}"
        );
    }
    let (tools, angle) = (bounds(cx, "model-tools"), bounds(cx, "model-angle"));
    assert!(
        angle.left() > bounds(cx, "model-mode-key-1").right() && angle.right() <= tools.right(),
        "角度 is beside the mode, on the leading plate"
    );
    assert!(
        bounds(cx, "model-slice-layer").bottom() <= bounds(cx, "model-slice").top(),
        "the layer field stands over 切片"
    );
    let (view, picture) = (bounds(cx, "model-tools-view"), bounds(cx, "model-viewport"));
    assert!(
        tools.right() < view.left() && view.right() <= picture.right(),
        "apart and inside: {tools:?} {view:?} in {picture:?}"
    );
    let clearance = viewer.read_with(cx, |viewer, _| viewer.tool_clearance());
    assert_eq!(clearance, SLICING_CLEARANCE);
    assert!(
        picture.bottom() - view.top() <= px(clearance),
        "what the read-out clears covers the taller plate"
    );
}

/// T25: 切片 starts at the top layer; the layer field's − and +, a layer
/// typed into it and `[` `]` move through the layers and stop at the
/// schematic's first and last; the field follows however the layer moved;
/// another axis starts at its top; each change is told to the worker.
#[gpui::test]
fn a_slice_steps_through_the_schematic_s_layers(cx: &mut TestAppContext) {
    use gpui_component::input::{InputEvent, NumberInputEvent, StepAction};
    use lumilio_schematic_render::{Slice, SliceAxis};
    let (viewer, mailbox, cx) = loaded_inline(cx, ([0, 0, 0], [3, 5, 2]));
    let slices = |mailbox: &Mailbox| {
        std::iter::from_fn(|| mailbox.try_take())
            .filter_map(|work| match work {
                Work::Job(Job::Slice { slice, .. }) => Some(slice),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let at = |axis, layer| Some(Slice { axis, layer });
    let layer_of = |cx: &mut gpui::VisualTestContext| {
        viewer.read_with(cx, |viewer, _| viewer.slice().map(|slice| slice.layer))
    };
    slices(&mailbox);
    assert!(cx.debug_bounds("model-slice-layer").is_none());
    choose_in_dropdown(&viewer, 2, cx);
    assert_eq!(
        viewer.read_with(cx, |viewer, _| viewer.slice()),
        at(SliceAxis::Y, 5)
    );
    let field = viewer
        .read_with(cx, |viewer, _| viewer.layer_field_for_test())
        .expect("the layer field");
    // The field's −.
    field.update(cx, |_, cx| {
        cx.emit(NumberInputEvent::Step(StepAction::Decrement))
    });
    cx.run_until_parked();
    assert_eq!(layer_of(cx), Some(4));
    assert_eq!(layer_text(&viewer, cx), "4");
    // `[` and `]` on the focused picture; the field follows.
    let at_key = cx.debug_bounds("model-viewport").unwrap().center();
    cx.simulate_click(at_key, Modifiers::none());
    cx.simulate_keystrokes("[ [ ]");
    assert_eq!(layer_of(cx), Some(3));
    assert_eq!(layer_text(&viewer, cx), "3");
    // A layer typed in; one past the top stops at the top.
    for (typed, layer) in [("1", 1), ("99", 5)] {
        cx.update(|window, cx| {
            field.update(cx, |field, cx| {
                field.set_value(typed, window, cx);
                cx.emit(InputEvent::Change);
            })
        });
        cx.run_until_parked();
        assert_eq!(layer_of(cx), Some(layer), "typed {typed}");
    }
    assert_eq!(
        layer_text(&viewer, cx),
        "5",
        "the field shows the layer it stopped at"
    );
    // Keys typed into the field are the field's: they neither zoom, turn
    // nor reset the picture.
    viewer.update(cx, |viewer, _| viewer.view = viewer.view.orbit(30., 0.));
    let view = viewer.read_with(cx, |viewer, _| viewer.view);
    cx.update(|window, cx| {
        use gpui::Focusable as _;
        field.read(cx).focus_handle(cx).focus(window, cx)
    });
    cx.simulate_keystrokes("- = r left");
    cx.run_until_parked();
    assert_eq!(viewer.read_with(cx, |viewer, _| viewer.view), view);
    viewer.update(cx, |viewer, cx| {
        viewer.step_layer(-10, cx);
        viewer.step_layer(-1, cx);
        assert_eq!(viewer.slice(), at(SliceAxis::Y, 0), "stops at the first");
    });
    choose_in_dropdown(&viewer, 1, cx);
    assert_eq!(
        viewer.read_with(cx, |viewer, _| viewer.slice()),
        at(SliceAxis::X, 3),
        "X starts at its top"
    );
    choose_in_dropdown(&viewer, 0, cx);
    assert_eq!(viewer.read_with(cx, |viewer, _| viewer.slice()), None);
    assert!(cx.debug_bounds("model-slice-layer").is_none());
    assert_eq!(
        slices(&mailbox),
        [
            at(SliceAxis::Y, 5),
            at(SliceAxis::Y, 4),
            at(SliceAxis::Y, 3),
            at(SliceAxis::Y, 2),
            at(SliceAxis::Y, 3),
            at(SliceAxis::Y, 1),
            at(SliceAxis::Y, 5),
            at(SliceAxis::Y, 0),
            at(SliceAxis::X, 3),
            None,
        ],
        "only changes are told"
    );
}

/// T25: a drag with Ctrl from one block to another selects the box between
/// them, both corners included, without turning the camera; the worker is
/// told and counts its blocks; Esc clears it.
#[gpui::test]
fn a_ctrl_drag_selects_the_box_between_two_blocks(cx: &mut TestAppContext) {
    let (viewer, mailbox, cx) = loaded_inline(cx, ([0, 0, 0], [3, 5, 2]));
    let yaw = viewer.read_with(cx, |viewer, _| viewer.view.yaw_deg);
    while mailbox.try_take().is_some() {}
    let answer_pick = |mailbox: &Mailbox, position: [i32; 3]| {
        let reply = std::iter::from_fn(|| mailbox.try_take())
            .find_map(|work| match work {
                Work::Job(Job::Pick { reply, .. }) => Some(reply),
                _ => None,
            })
            .expect("a pick is asked");
        reply
            .send_blocking(Some(lumilio_schematic_render::Picked {
                position,
                state: "minecraft:stone".into(),
            }))
            .unwrap();
    };
    let viewport = cx.debug_bounds("model-viewport").unwrap();
    let (from, to) = (
        viewport.center(),
        viewport.center() + point(px(40.), px(20.)),
    );
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::control());
    answer_pick(&mailbox, [3, 0, 2]);
    cx.run_until_parked();
    cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::control());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::control());
    answer_pick(&mailbox, [1, 2, 0]);
    cx.run_until_parked();
    let selected = Some(([1, 0, 0], [3, 2, 2]));
    viewer.read_with(cx, |viewer, _| {
        assert_eq!(viewer.selection(), selected);
        assert_eq!(viewer.view.yaw_deg, yaw, "the camera stays");
        assert!(viewer.inspected().is_none(), "nothing is inspected");
    });
    let mut told = None;
    for work in std::iter::from_fn(|| mailbox.try_take()) {
        match work {
            Work::Job(Job::Selection { cells, .. }) => told = Some(cells),
            Work::Job(Job::BlocksIn { min, max, reply }) => {
                assert_eq!(Some((min, max)), selected);
                reply
                    .send_blocking(vec![("minecraft:stone".into(), 9)])
                    .unwrap();
            }
            _ => {}
        }
    }
    assert_eq!(told, Some(selected), "the worker outlines it");
    cx.run_until_parked();
    viewer.read_with(cx, |viewer, _| {
        assert_eq!(
            viewer.selection_blocks(),
            [("minecraft:stone".to_owned(), 9)]
        );
    });
    cx.simulate_keystrokes("escape");
    viewer.read_with(cx, |viewer, _| assert_eq!(viewer.selection(), None));
    let cleared = std::iter::from_fn(|| mailbox.try_take())
        .any(|work| matches!(work, Work::Job(Job::Selection { cells: None, .. })));
    assert!(cleared, "the outline goes");
}
