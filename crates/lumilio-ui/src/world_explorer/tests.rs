use super::camera::Camera;
use super::*;
use gpui::{Modifiers, TestAppContext, point};
use lumilio_plugin_api::map::{BaseMapInfo, Dimension, WorldId};
#[test]
fn zoom_keeps_pointer_world_coordinate_and_lod_uses_fourfold_steps() {
    let mut camera = Camera::default();
    let before = camera.world([100., 50.], [800, 600]);
    camera.zoom(0.5, [100., 50.], [800, 600]);
    assert_eq!(camera.world([100., 50.], [800, 600]), before);
    // Level changes where a tile texel is twice a screen pixel, so a level is
    // never generated finer than it can be told apart.
    for (scale, level) in [
        (0.25, 0),
        (1.99, 0),
        (2., 1),
        (7.9, 1),
        (8., 2),
        (16., 2),
        (32., 3),
        (128., 4),
        (256., 4),
    ] {
        camera.scale = scale;
        assert_eq!(camera.level(), level, "{scale}");
    }
}

#[gpui::test]
fn pointer_keyboard_and_dimension_controls_preserve_camera_and_reject_departed_tiles(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_component::init);
    let (view, cx) = cx.add_window_view(|_, cx| MapView::new(Rc::new(|_, _, _| {}), cx));
    let (send, commands) = async_channel::bounded(128);
    let (_events, receive) = async_channel::bounded(128);
    view.update(cx, |view, cx| {
        view.connection = Some(Connection { send, receive });
        view.context = Some(WorldContext {
            world: WorldId::Seed {
                seed: 262,
                version: "1.21.4".into(),
            },
            seed: Some(262),
            version: Some("1.21.4".into()),
            data_version: None,
            dimension: Dimension::Overworld,
            sources: vec![],
        });
        view.providers.base_maps.push((
            "test.seed".into(),
            BaseMapInfo {
                id: "seed".into(),
                kind_id: "map-base-seed".into(),
                dimensions: vec![Dimension::Overworld, Dimension::Nether, Dimension::End],
                levels: vec![0, 1, 2, 3, 4],
            },
        ));
        let mut other = view.providers.base_maps[0].clone();
        other.1.id = "other".into();
        view.providers.base_maps.push(other);
        cx.notify();
    });
    cx.run_until_parked();
    let center = cx.debug_bounds("world-map").unwrap().center();
    cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        center + point(px(25.), px(10.)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    cx.simulate_mouse_up(center, MouseButton::Left, Modifiers::none());
    view.read_with(cx, |view, _| {
        assert_eq!(view.camera.x, -100.);
        assert_eq!(view.camera.z, -40.);
    });
    cx.simulate_keystrokes("right");
    view.read_with(cx, |view, _| assert_eq!(view.camera.x, 28.));
    cx.simulate_keystrokes("-");
    let (camera, generation, key) = view.update(cx, |view, _| {
        assert_eq!(view.camera.scale, 5.);
        let key = view.visible[0].clone();
        // Capture an actual queued request, rather than a scheduler self-report.
        let mut generation = None;
        while let Ok(command) = commands.try_recv() {
            if let Command::Tile {
                generation: current,
                request,
                ..
            } = command
                && request.key == key
            {
                generation = Some(current);
            }
        }
        (view.camera, generation.unwrap(), key)
    });
    cx.run_until_parked();
    let nether = cx.debug_bounds("map-dimension-key-1").unwrap().center();
    cx.simulate_click(nether, Modifiers::none());
    view.update(cx, |view, cx| {
        assert_eq!(view.camera, camera);
        assert_eq!(view.context.as_ref().unwrap().dimension, Dimension::Nether);
        view.event(
            Event::Tile {
                generation,
                key: key.clone(),
                result: Ok(TileReply::Empty),
            },
            cx,
        );
        assert!(!view.tiles.contains_key(&key));
    });
    click(cx, "map-base-key-1");
    view.read_with(cx, |view, _| {
        assert_eq!(view.base, 1);
        assert_eq!(view.camera, camera);
        assert_eq!(view.context.as_ref().unwrap().dimension, Dimension::Nether);
    });
    let reopened = cx.new(|cx| MapView::new(Rc::new(|_, _, _| {}), cx));
    reopened.update(cx, |view, cx| {
        view.event(
            Event::Tile {
                generation,
                key,
                result: Ok(TileReply::Empty),
            },
            cx,
        );
        assert!(view.tiles.is_empty());
    });
}

fn rooted_map(
    cx: &mut TestAppContext,
    mode: gpui_component::ThemeMode,
) -> (gpui::Entity<MapView>, &mut gpui::VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        gpui_component::Theme::change(mode, None, cx);
        crate::theme::tune(cx);
        cx.set_reduce_motion(true);
    });
    let slot = Rc::new(std::cell::RefCell::new(None));
    let keep = slot.clone();
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| MapView::new(Rc::new(|_, _, _| {}), cx));
        *keep.borrow_mut() = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    let view = slot.borrow().clone().unwrap();
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    (view, cx)
}

fn click(cx: &mut gpui::VisualTestContext, selector: &'static str) {
    cx.run_until_parked();
    let at = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("missing {selector}"))
        .center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

fn connect_commands(
    view: &gpui::Entity<MapView>,
    cx: &mut gpui::VisualTestContext,
) -> async_channel::Receiver<Command> {
    let (send, commands) = async_channel::bounded(256);
    let (_events, receive) = async_channel::bounded(256);
    view.update(cx, |view, _| {
        view.connection = Some(Connection { send, receive })
    });
    commands
}

fn next_seed(commands: &async_channel::Receiver<Command>) -> (i64, String) {
    while let Ok(command) = commands.try_recv() {
        if let Command::SaveSeed { seed, version } = command {
            return (seed, version);
        }
    }
    panic!("no seed was submitted")
}

#[gpui::test]
fn inline_seed_enter_and_blur_apply_without_map_stealing_typing(cx: &mut TestAppContext) {
    let (view, cx) = rooted_map(cx, gpui_component::ThemeMode::Light);
    let commands = connect_commands(&view, cx);
    assert!(cx.debug_bounds("map-world-select").is_none());
    click(cx, "map-seed-input");
    cx.simulate_input("-262");
    cx.simulate_keystrokes("left");
    cx.run_until_parked();
    view.read_with(cx, |view, cx| {
        assert_eq!(view.camera, Camera::default());
        let form = view.form.as_ref().unwrap();
        assert_eq!(form.seed.read(cx).value().as_str(), "-262");
        assert!(form.seed_dirty);
    });
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert!(view.form.as_ref().unwrap().submitted.is_some())
    });
    assert_eq!(next_seed(&commands), (-262, "1.21.4".into()));
    let input = view.read_with(cx, |view, _| view.form.as_ref().unwrap().seed.clone());
    cx.update(|window, cx| input.update(cx, |input, cx| input.set_value("", window, cx)));
    cx.simulate_input("hello");
    cx.run_until_parked();
    view.read_with(cx, |view, cx| {
        let form = view.form.as_ref().unwrap();
        assert_eq!(form.seed.read(cx).value().as_str(), "hello");
        assert!(form.seed_dirty);
    });
    click(cx, "map-jump-x");
    cx.update(|window, cx| {
        view.read_with(cx, |view, cx| {
            use gpui::Focusable as _;
            let form = view.form.as_ref().unwrap();
            assert!(
                form.jump_x.read(cx).focus_handle(cx).is_focused(window),
                "jump should take focus"
            );
            assert!(
                !form.seed.read(cx).focus_handle(cx).is_focused(window),
                "seed should lose focus"
            );
            assert!(!form.seed_dirty, "blur must apply the draft");
        })
    });
    assert_eq!(next_seed(&commands), (99_162_322, "1.21.4".into()));
    assert!(cx.debug_bounds("map-seed-error").is_none());
    let context = WorldContext {
        world: WorldId::Seed {
            seed: 99_162_322,
            version: "1.21.4".into(),
        },
        seed: Some(99_162_322),
        version: Some("1.21.4".into()),
        data_version: None,
        dimension: Dimension::Overworld,
        sources: vec![],
    };
    view.update(cx, |view, cx| {
        view.event(
            Event::Seed(Ok((
                vec![WorldMapContext {
                    name: "manual".into(),
                    context: context.clone(),
                    spawn: None,
                }],
                context,
            ))),
            cx,
        )
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("map-world-select").is_none(),
        "manual seeds are not saves"
    );
    view.read_with(cx, |view, cx| {
        assert_eq!(
            view.form.as_ref().unwrap().seed.read(cx).value().as_str(),
            "hello"
        )
    });
    click(cx, "map-seed-input");
    cx.update(|window, cx| input.update(cx, |input, cx| input.set_value("", window, cx)));
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(cx.debug_bounds("map-seed-error").is_some());
}

fn saved_world(folder: &str, version: &str, seed: i64) -> WorldMapContext {
    WorldMapContext {
        name: folder.to_owned(),
        spawn: None,
        context: WorldContext {
            world: WorldId::Save {
                instance: "test".into(),
                folder: folder.into(),
            },
            seed: Some(seed),
            version: Some(version.into()),
            data_version: None,
            dimension: Dimension::Overworld,
            sources: vec![],
        },
    }
}

#[gpui::test]
fn searchable_world_and_version_selects_follow_supported_defaults(cx: &mut TestAppContext) {
    let (view, cx) = rooted_map(cx, gpui_component::ThemeMode::Light);
    let commands = connect_commands(&view, cx);
    view.update(cx, |view, cx| {
        view.event(
            Event::Contexts(Ok((
                vec![
                    saved_world("First", "1.18.2", 262),
                    saved_world("Second", "26.3", 263),
                ],
                MapProviders::default(),
            ))),
            cx,
        )
    });
    cx.run_until_parked();
    view.read_with(cx, |view, cx| {
        let form = view.form.as_ref().unwrap();
        assert_eq!(
            form.version.read(cx).selected_value().map(String::as_str),
            Some("1.18.2")
        );
        assert_eq!(
            form.world.read(cx).selected_value(),
            Some(&view.context.as_ref().unwrap().world)
        );
    });
    click(cx, "map-world-select");
    cx.simulate_input("Second");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |view, cx| {
        assert_eq!(
            view.context.as_ref().unwrap().version.as_deref(),
            Some("26.3")
        );
        assert_eq!(
            view.form
                .as_ref()
                .unwrap()
                .version
                .read(cx)
                .selected_value()
                .map(String::as_str),
            Some("1.21.4")
        );
    });
    click(cx, "map-version-select");
    cx.simulate_input("1.16.5");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(next_seed(&commands), (263, "1.16.5".into()));
    view.read_with(cx, |view, cx| {
        assert_eq!(
            view.form
                .as_ref()
                .unwrap()
                .version
                .read(cx)
                .selected_value()
                .map(String::as_str),
            Some("1.16.5")
        );
    });
}

#[test]
fn jump_axis_accepts_numbers_inside_the_border_only() {
    for (text, value) in [
        ("-12", -12.),
        (" 34 ", 34.),
        ("0.5", 0.5),
        ("29900000", 29_900_000.),
    ] {
        assert_eq!(seed::axis(text), Some(value), "{text}");
    }
    for text in [
        "",
        "  ",
        "1 2",
        "1,2",
        "NaN",
        "inf",
        "30000000",
        "-30000000",
        "word",
    ] {
        assert_eq!(seed::axis(text), None, "{text}");
    }
}

#[gpui::test]
fn go_key_and_enter_move_camera_and_a_bad_axis_shows_inline_error(cx: &mut TestAppContext) {
    let (view, cx) = rooted_map(cx, gpui_component::ThemeMode::Dark);
    let (x, z) = view.read_with(cx, |view, _| {
        let form = view.form.as_ref().unwrap();
        (form.jump_x.clone(), form.jump_z.clone())
    });
    let set = |cx: &mut gpui::VisualTestContext, x_text: &str, z_text: &str| {
        cx.update(|window, cx| {
            x.update(cx, |input, cx| input.set_value(x_text, window, cx));
            z.update(cx, |input, cx| input.set_value(z_text, window, cx));
        });
        cx.run_until_parked();
    };
    set(cx, "-12", "34");
    click(cx, "map-go");
    view.read_with(cx, |view, _| {
        assert_eq!([view.camera.x, view.camera.z], [-12., 34.])
    });
    assert!(cx.debug_bounds("map-jump-error").is_none());
    set(cx, "-56", "-78");
    click(cx, "map-jump-z");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!([view.camera.x, view.camera.z], [-56., -78.])
    });
    // One bad axis moves nothing; fixing the field clears the message.
    set(cx, "oops", "5");
    click(cx, "map-go");
    view.read_with(cx, |view, _| {
        assert!(view.form.as_ref().unwrap().jump_error);
        assert_eq!([view.camera.x, view.camera.z], [-56., -78.]);
    });
    assert!(cx.debug_bounds("map-jump-error").is_some());
    // Typing into a field is an edit; the stale message goes away.
    click(cx, "map-jump-x");
    cx.simulate_input("1");
    cx.run_until_parked();
    assert!(cx.debug_bounds("map-jump-error").is_none());
}

#[gpui::test]
fn layers_popover_switches_show_state_and_coarse_zoom_hint_in_both_themes(cx: &mut TestAppContext) {
    for mode in [
        gpui_component::ThemeMode::Light,
        gpui_component::ThemeMode::Dark,
    ] {
        let (view, cx) = rooted_map(cx, mode);
        view.update(cx, |view, cx| {
            view.camera.scale = 16.;
            cx.notify();
        });
        click(cx, "map-layers");
        assert!(cx.debug_bounds("map-layer-panel").is_some());
        assert!(cx.debug_bounds("map-chunks-hint").is_some());
        let off = cx.debug_bounds("map-chunks-cap").unwrap().origin.x;
        click(cx, "map-chunks-toggle");
        view.read_with(cx, |view, _| assert!(view.layers.chunks));
        let on = cx.debug_bounds("map-chunks-cap").unwrap().origin.x;
        assert!(on > off, "switch cap must visibly move");
        click(cx, "map-regions-toggle");
        view.read_with(cx, |view, _| assert!(view.layers.regions));
        view.update(cx, |view, cx| {
            view.camera.scale = 4.;
            cx.notify();
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("map-chunks-hint").is_none());
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(cx.debug_bounds("map-layer-panel").is_none());
        view.read_with(cx, |view, _| assert_eq!(view.camera.scale, 4.));
    }
}

#[gpui::test]
fn retry_uses_one_counted_button_and_viewport_receives_surplus_height(cx: &mut TestAppContext) {
    let (view, cx) = rooted_map(cx, gpui_component::ThemeMode::Light);
    assert!(cx.debug_bounds("map-retry-0").is_none());
    view.update(cx, |view, cx| {
        let template = TileKey {
            provider: "test.seed".into(),
            base_map: "seed".into(),
            world: WorldId::Seed {
                seed: 262,
                version: "1.21.4".into(),
            },
            dimension: Dimension::Overworld,
            level: 0,
            tx: 0,
            tz: 0,
        };
        for tx in [0, 1, 2] {
            view.failed.insert(
                TileKey {
                    tx,
                    ..template.clone()
                },
                MapFailure::Failed("test".into()),
            );
        }
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("map-retry-3").is_some());
    assert!(cx.debug_bounds("map-retry-2").is_none());
    click(cx, "map-retry-3");
    view.read_with(cx, |view, _| assert!(view.failed.is_empty()));
    assert!(cx.debug_bounds("map-retry-3").is_none());
    cx.simulate_resize(gpui::size(px(1080.), px(720.)));
    cx.run_until_parked();
    let small = cx.debug_bounds("world-map").unwrap();
    cx.simulate_resize(gpui::size(px(1080.), px(920.)));
    cx.run_until_parked();
    let tall = cx.debug_bounds("world-map").unwrap();
    assert_eq!(tall.size.height - small.size.height, px(200.));
}

fn tile_key(level: u8, tx: i32, tz: i32) -> TileKey {
    TileKey {
        provider: "test.seed".into(),
        base_map: "seed".into(),
        world: WorldId::Seed {
            seed: 262,
            version: "1.21.4".into(),
        },
        dimension: Dimension::Overworld,
        level,
        tx,
        tz,
    }
}

fn ready_view(
    cx: &mut TestAppContext,
) -> (
    gpui::Entity<MapView>,
    &mut gpui::VisualTestContext,
    async_channel::Receiver<Command>,
) {
    let (view, cx) = rooted_map(cx, gpui_component::ThemeMode::Light);
    let commands = connect_commands(&view, cx);
    view.update(cx, |view, cx| {
        view.context = Some(WorldContext {
            world: tile_key(0, 0, 0).world,
            seed: Some(262),
            version: Some("1.21.4".into()),
            data_version: None,
            dimension: Dimension::Overworld,
            sources: vec![],
        });
        view.providers.base_maps.push((
            "test.seed".into(),
            BaseMapInfo {
                id: "seed".into(),
                kind_id: "map-base-seed".into(),
                dimensions: vec![Dimension::Overworld],
                levels: vec![0, 1, 2, 3, 4],
            },
        ));
        cx.notify();
    });
    cx.run_until_parked();
    (view, cx, commands)
}

fn image(shade: u8) -> TileReply {
    TileReply::Image(lumilio_plugin_api::ImageData {
        width: 256,
        height: 256,
        rgba: [shade, shade, shade, 255].repeat(256 * 256),
    })
}

#[gpui::test]
fn dispatch_keeps_a_bounded_queue_and_refills_it_as_tiles_arrive(cx: &mut TestAppContext) {
    let (view, cx, commands) = ready_view(cx);
    // A wide, zoomed-out window wants far more tiles than may be in flight.
    cx.simulate_resize(gpui::size(px(1900.), px(1100.)));
    cx.run_until_parked();
    let sent = |commands: &async_channel::Receiver<Command>| {
        let mut keys = vec![];
        while let Ok(command) = commands.try_recv() {
            if let Command::Tile {
                request,
                generation,
                ..
            } = command
            {
                keys.push((generation, request.key));
            }
        }
        keys
    };
    let first = sent(&commands);
    let wanted = view.read_with(cx, |view, _| view.visible.len());
    assert!(wanted > IN_FLIGHT, "scenario must exceed the cap: {wanted}");
    assert_eq!(first.len(), IN_FLIGHT);
    view.update(cx, |view, cx| {
        let (generation, key) = first[0].clone();
        view.event(
            Event::Tile {
                generation,
                key,
                result: Ok(image(10)),
            },
            cx,
        );
    });
    assert_eq!(sent(&commands).len(), 1, "one finished, one more starts");
}

#[gpui::test]
fn finished_tiles_of_another_level_stand_in_and_are_evicted_oldest_first(cx: &mut TestAppContext) {
    let (view, cx, _commands) = ready_view(cx);
    cx.simulate_resize(gpui::size(px(800.), px(600.)));
    cx.run_until_parked();
    view.update(cx, |view, _| {
        let visible = view.visible.clone();
        assert!(!visible.is_empty());
        let level = visible[0].level;
        // A cached coarser tile over the viewport centre.
        let coarse = tile_key(level + 1, 0, 0);
        view.revision += 1;
        view.tiles.insert(
            coarse.clone(),
            Loaded {
                rgba: prepare(image(7)),
                revision: view.revision,
                used: 0,
            },
        );
        let ids: Vec<String> = view.scene_tiles().into_iter().map(|tile| tile.id).collect();
        // Patterns for the unloaded visible tiles first, the stand-in on top of
        // them; nothing else is cached to draw.
        assert_eq!(ids.len(), visible.len() + 1, "{ids:?}");
        assert!(ids[..visible.len()].iter().all(|id| id == "pending"));
        assert_eq!(ids[visible.len()], format!("tile-{}", view.revision));
        // Fill past the cap with old, off-screen tiles.
        for n in 0..(TILE_CAP as i32 + 40) {
            view.revision += 1;
            view.tiles.insert(
                tile_key(0, 10_000 + n, 10_000),
                Loaded {
                    rgba: None,
                    revision: view.revision,
                    used: n as u64 + 1,
                },
            );
        }
        view.evict();
        assert_eq!(view.tiles.len(), TILE_CAP);
        assert!(
            view.tiles
                .contains_key(&tile_key(0, 10_000 + 40 + 1, 10_000)),
            "newer survive"
        );
        assert!(
            !view.tiles.contains_key(&tile_key(0, 10_000, 10_000)),
            "oldest go first"
        );
    });
}
