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

fn village_layer() -> (String, OverlayInfo) {
    (
        "test.seed".into(),
        OverlayInfo {
            id: "structure.village".into(),
            kind_id: "map-structure-village".into(),
            dimensions: vec![Dimension::Overworld],
            icon: Some(lumilio_plugin_api::map::MapIcon::Village),
            group_id: Some("map-group-structures".into()),
            approximate: false,
        },
    )
}

fn icon_object(x: f64, z: f64, priority: i32) -> MapObject {
    MapObject {
        id: format!("structure.village:{x}:{z}"),
        raw_id: format!("{x},{z}"),
        source: "test.seed".into(),
        world: tile_key(0, 0, 0).world,
        dimension: Dimension::Overworld,
        kind: MapObjectKind::Icon {
            icon: lumilio_plugin_api::map::MapIcon::Village,
            at: lumilio_plugin_api::map::MapPoint { x, z },
        },
        label: None,
        label_id: Some("map-structure-village".into()),
        priority,
        approximate: false,
        color: None,
    }
}

fn world_context() -> WorldContext {
    WorldContext {
        world: tile_key(0, 0, 0).world,
        seed: Some(262),
        version: Some("1.21.4".into()),
        data_version: None,
        dimension: Dimension::Overworld,
        sources: vec![],
    }
}

#[test]
fn object_cells_are_wanted_nearest_first_capped_in_flight_and_dropped_when_stale() {
    let mut objects = objects::Objects::new(&["structure.village"]);
    objects.layers = vec![village_layer()];
    let context = world_context();
    // A view straddling the origin touches four 4096-block cells.
    objects.update(&context, [-100., -100., 100., 100.], 1.);
    let first = objects.next(&context);
    assert_eq!(first.len(), 4);
    assert!(
        objects.next(&context).is_empty(),
        "nothing new while all are in flight"
    );
    // Bounds are the cell, half-open, so a structure belongs to exactly one.
    let spans: std::collections::BTreeSet<_> = first
        .iter()
        .map(|next| {
            let bounds = next.request.bounds;
            (
                bounds.min.x as i64,
                bounds.min.z as i64,
                (bounds.max.x - bounds.min.x) as i64,
            )
        })
        .collect();
    assert_eq!(
        spans,
        [
            (-4096, -4096, 4096),
            (-4096, 0, 4096),
            (0, -4096, 4096),
            (0, 0, 4096)
        ]
        .into_iter()
        .collect()
    );
    // Moving away cancels what left view; the old answers are refused.
    objects.update(&context, [20_000., 20_000., 20_100., 20_100.], 1.);
    assert!(first.iter().all(|next| next.cancel.is_cancelled()));
    for next in &first {
        assert!(!objects.accept(next.generation, &next.key));
    }
    let second = objects.next(&context);
    assert_eq!(second.len(), 1);
    assert!(objects.accept(second[0].generation, &second[0].key));
    objects.store(
        second[0].key.clone(),
        vec![icon_object(20_050., 20_050., 0)],
    );
    assert_eq!(
        objects.visible([20_000., 20_000., 20_100., 20_100.]).len(),
        1
    );
    // Zoomed out past the icon limit nothing is wanted and nothing is drawn.
    objects.update(
        &context,
        [20_000., 20_000., 20_100., 20_100.],
        objects::MAX_SCALE + 1.,
    );
    assert!(objects.next(&context).is_empty());
    assert!(
        objects
            .visible([20_000., 20_000., 20_100., 20_100.])
            .is_empty()
    );
}

#[test]
fn clearing_objects_cancels_requests_and_refuses_late_answers() {
    let mut objects = objects::Objects::new(&["structure.village"]);
    objects.layers = vec![village_layer()];
    let context = world_context();
    objects.update(&context, [10., 10., 20., 20.], 1.);
    let started = objects.next(&context);
    assert_eq!(started.len(), 1);
    objects.clear();
    assert!(started[0].cancel.is_cancelled());
    assert!(!objects.accept(started[0].generation, &started[0].key));
    // A failed cell stays failed until retried.
    objects.layers = vec![village_layer()];
    objects.update(&context, [10., 10., 20., 20.], 1.);
    let again = objects.next(&context);
    assert!(objects.accept(again[0].generation, &again[0].key));
    objects.fail(again[0].key.clone());
    assert_eq!(objects.failed(), 1);
    assert!(objects.next(&context).is_empty());
    objects.retry();
    assert_eq!(objects.next(&context).len(), 1);
}

#[gpui::test]
fn structure_icons_follow_the_camera_and_the_layer_panel_toggles_them(cx: &mut TestAppContext) {
    let (view, cx, commands) = ready_view(cx);
    cx.simulate_resize(gpui::size(px(1000.), px(700.)));
    cx.run_until_parked();
    // Setting the world asks the plugins which layers it offers.
    view.update(cx, |view, _| view.context_changed());
    let mut asked = false;
    while let Ok(command) = commands.try_recv() {
        asked |= matches!(command, Command::Overlays { .. });
    }
    assert!(asked);
    let context = view.read_with(cx, |view, _| view.context.clone().unwrap());
    // An answer for another world is ignored.
    let mut other = context.clone();
    other.dimension = Dimension::Nether;
    view.update(cx, |view, cx| {
        view.event(
            Event::Overlays {
                context: other,
                layers: vec![village_layer()],
            },
            cx,
        )
    });
    view.read_with(cx, |view, _| assert!(view.objects.layers.is_empty()));
    view.update(cx, |view, cx| {
        view.event(
            Event::Overlays {
                context,
                layers: vec![village_layer()],
            },
            cx,
        )
    });
    // Villages are on by default, so the cells under the view are requested;
    // answer each, with one village at the origin in whichever cell holds it.
    let mut requested = vec![];
    while let Ok(command) = commands.try_recv() {
        if let Command::Objects {
            generation,
            key,
            request,
            plugin,
            ..
        } = command
        {
            assert_eq!(plugin, "test.seed");
            assert_eq!(request.overlay, "structure.village");
            requested.push((generation, key, request.bounds));
        }
    }
    assert!(
        !requested.is_empty() && requested.len() <= 4,
        "{}",
        requested.len()
    );
    view.update(cx, |view, cx| {
        for (generation, key, bounds) in requested {
            let holds =
                bounds.min.x <= 0. && 0. < bounds.max.x && bounds.min.z <= 0. && 0. < bounds.max.z;
            let found = if holds {
                vec![icon_object(0., 0., 0)]
            } else {
                vec![]
            };
            view.event(
                Event::Objects {
                    generation,
                    key,
                    result: Ok(found),
                },
                cx,
            );
        }
    });
    cx.run_until_parked();
    // The icon is a sprite in the map frame, centred where the camera looks.
    let sprites =
        |cx: &mut gpui::VisualTestContext| view.update(cx, |view, _| (view.sprites(), view.size));
    let (found, size) = sprites(cx);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, "village");
    assert_eq!(
        (found[0].x, found[0].y),
        (f64::from(size[0]) / 2., f64::from(size[1]) / 2.)
    );
    assert_eq!(
        (found[0].width, found[0].height, found[0].size),
        (80, 80, 28)
    );
    assert_eq!(found[0].rgba.len(), 80 * 80 * 4);
    // Panning right by 100 blocks at 4 blocks per pixel moves the icon 25 px left.
    view.update(cx, |view, cx| {
        view.camera.x = 100.;
        view.refresh();
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(sprites(cx).0[0].x, found[0].x - 25.);
    // The panel lists the layer with its icon; switching it off hides the icons.
    click(cx, "map-layers");
    assert!(cx.debug_bounds("map-layer-row-structure.village").is_some());
    assert!(cx.debug_bounds("map-estimated-note").is_none());
    click(cx, "map-layer-toggle-structure.village");
    view.read_with(cx, |view, _| {
        assert!(!view.objects.enabled.contains("structure.village"))
    });
    assert!(sprites(cx).0.is_empty());
    click(cx, "map-layer-toggle-structure.village");
    assert_eq!(sprites(cx).0.len(), 1, "cached cells come straight back");
    // Zoomed out the icons give way to an explanation.
    view.update(cx, |view, cx| {
        view.camera.scale = 32.;
        view.refresh();
        cx.notify();
    });
    cx.run_until_parked();
    assert!(sprites(cx).0.is_empty(), "no icons past the zoom limit");
    assert!(cx.debug_bounds("map-structures-hint").is_some());
}

#[gpui::test]
fn estimated_layers_are_tagged_and_explained(cx: &mut TestAppContext) {
    let (view, cx, _commands) = ready_view(cx);
    let mut layer = village_layer();
    layer.1.id = "structure.mansion".into();
    layer.1.kind_id = "map-structure-mansion".into();
    layer.1.approximate = true;
    view.update(cx, |view, cx| {
        view.objects.layers = vec![layer];
        cx.notify();
    });
    click(cx, "map-layers");
    assert!(
        cx.debug_bounds("map-layer-estimated-structure.mansion")
            .is_some()
    );
    assert!(cx.debug_bounds("map-estimated-note").is_some());
}

#[gpui::test]
fn icons_arrive_through_a_live_connection_after_the_world_is_chosen(cx: &mut TestAppContext) {
    let (view, cx) = rooted_map(cx, gpui_component::ThemeMode::Dark);
    let (send, commands) = async_channel::bounded(128);
    let (events, receive) = async_channel::bounded(128);
    let world = world_context();
    // A stand-in for the application's command loop, answering like it does.
    let answer_world = world.clone();
    cx.executor()
        .spawn(async move {
            while let Ok(command) = commands.recv().await {
                let event = match command {
                    Command::Contexts => Event::Contexts(Ok((
                        vec![WorldMapContext {
                            name: "manual".into(),
                            context: answer_world.clone(),
                            spawn: None,
                        }],
                        MapProviders {
                            base_maps: vec![(
                                "test.seed".into(),
                                BaseMapInfo {
                                    id: "seed".into(),
                                    kind_id: "map-base-seed".into(),
                                    dimensions: vec![Dimension::Overworld],
                                    levels: vec![0, 1, 2, 3, 4],
                                },
                            )],
                            overlays: vec![],
                        },
                    ))),
                    Command::Overlays { context } => Event::Overlays {
                        context,
                        layers: vec![village_layer()],
                    },
                    Command::Objects {
                        generation,
                        key,
                        request,
                        ..
                    } => {
                        let b = request.bounds;
                        let holds =
                            b.min.x <= 100. && 100. < b.max.x && b.min.z <= 50. && 50. < b.max.z;
                        Event::Objects {
                            generation,
                            key,
                            result: Ok(if holds {
                                vec![icon_object(100., 50., 0)]
                            } else {
                                vec![]
                            }),
                        }
                    }
                    Command::Tile {
                        generation,
                        request,
                        ..
                    } => Event::Tile {
                        generation,
                        key: request.key,
                        result: Ok(TileReply::Empty),
                    },
                    Command::SaveSeed { .. } => continue,
                };
                let _ = events.send(event).await;
            }
        })
        .detach();
    view.update(cx, |view, cx| view.attach(Connection { send, receive }, cx));
    cx.run_until_parked();
    cx.simulate_resize(gpui::size(px(1000.), px(700.)));
    cx.run_until_parked();
    cx.executor().run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(view.objects.layers.len(), 1, "the catalog arrived");
        assert!(
            view.objects.visible(view.view_blocks()).len() == 1,
            "the village arrived"
        );
    });
    let found = view.update(cx, |view, _| view.sprites());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, "village");
    // The village is at block (100, 50), 25 and 12.5 px from the centre.
    let size = view.read_with(cx, |view, _| view.size);
    assert_eq!(
        (found[0].x, found[0].y),
        (
            f64::from(size[0]) / 2. + 25.,
            f64::from(size[1]) / 2. + 12.5
        )
    );
    // The real icon, through the real renderer: the frame differs from the
    // bare map inside the icon's 28 px box and nowhere else.
    let Ok(mut scene) = lumilio_map_render::Scene::new() else {
        return;
    };
    let camera = view.read_with(cx, |view, _| lumilio_map_render::Camera {
        x: view.camera.x,
        z: view.camera.z,
        blocks_per_pixel: view.camera.scale,
    });
    let mut render = |sprites: &[lumilio_map_render::Sprite]| {
        scene
            .render(
                camera,
                &[],
                sprites,
                lumilio_map_render::Grid::default(),
                size[0],
                size[1],
            )
            .unwrap()
    };
    let bare = render(&[]);
    let drawn = render(&found);
    let differs = |x: u32, y: u32| {
        let at = ((y * size[0] + x) * 4) as usize;
        bare.bgra[at..at + 4] != drawn.bgra[at..at + 4]
    };
    let (x0, y0) = (found[0].x as u32, found[0].y as u32);
    let inside = (y0 - 14..y0 + 14)
        .flat_map(|y| (x0 - 14..x0 + 14).map(move |x| (x, y)))
        .filter(|(x, y)| differs(*x, *y))
        .count();
    assert!(inside > 300, "the icon covers most of its box: {inside}");
    assert!(!differs(x0 + 40, y0), "nothing is drawn beside it");
    assert!(!differs(x0, y0 + 40));
}

// ---- the GPUI canvas backend -------------------------------------------------

fn flat_tile(tx: i32, tz: i32, level: u8) -> lumilio_map_render::Tile {
    let span = f64::from(tile_key(level, tx, tz).blocks_per_pixel().unwrap()) * 256.;
    lumilio_map_render::Tile {
        id: format!("t{tx},{tz},{level}"),
        x: f64::from(tx) * span,
        z: f64::from(tz) * span,
        span,
        rgba: [10, 20, 30, 255].repeat(256 * 256).into(),
    }
}

#[test]
fn neighbouring_tiles_share_their_edge_exactly_at_any_camera() {
    // Gaps and overlaps between tiles come from an edge computed two ways. Each
    // edge here comes from one block coordinate, so the shared one must match
    // bit for bit, whatever the zoom, the pan or the sign.
    let mut checked = 0;
    for level in 0..=4 {
        for scale in [0.25, 0.7, 1., 1.99, 3.7, 4., 5.5, 16., 33.3, 256.] {
            for (cx, cz) in [(0., 0.), (123.456, -987.654), (-29_999_000.5, 1.0e7 + 0.3)] {
                for (w, h) in [(802., 275.), (1000.5, 700.25)] {
                    let camera = lumilio_map_render::Camera {
                        x: cx,
                        z: cz,
                        blocks_per_pixel: scale,
                    };
                    for (tx, tz) in [(-3, -2), (0, 0), (5, 7), (-1, 4)] {
                        let here = canvas::tile_edges(&camera, w, h, &flat_tile(tx, tz, level));
                        let east = canvas::tile_edges(&camera, w, h, &flat_tile(tx + 1, tz, level));
                        let south =
                            canvas::tile_edges(&camera, w, h, &flat_tile(tx, tz + 1, level));
                        assert_eq!(here[2].to_bits(), east[0].to_bits(), "{level} {scale} {tx}");
                        assert_eq!(
                            here[3].to_bits(),
                            south[1].to_bits(),
                            "{level} {scale} {tz}"
                        );
                        checked += 1;
                    }
                }
            }
        }
    }
    assert_eq!(checked, 5 * 10 * 3 * 2 * 4);
    // And a tile's width on screen is its span over the zoom, 256 texels' worth.
    let camera = lumilio_map_render::Camera {
        x: 0.,
        z: 0.,
        blocks_per_pixel: 4.,
    };
    let edges = canvas::tile_edges(&camera, 800., 600., &flat_tile(0, 0, 1));
    assert_eq!(edges, [400., 300., 656., 556.]);
}

#[test]
fn a_tile_image_is_bgra_with_a_replicated_border() {
    // Distinct corner pixels: top-left (1,2,3,255) and bottom-right (7,8,9,255).
    let mut rgba = [10, 20, 30, 255].repeat(256 * 256);
    rgba[..4].copy_from_slice(&[1, 2, 3, 255]);
    rgba[(256 * 256 - 1) * 4..].copy_from_slice(&[7, 8, 9, 255]);
    let image = canvas::tile_image(&rgba);
    assert_eq!(image.size(0).width.0, 258);
    assert_eq!(image.size(0).height.0, 258);
    let bytes = image.as_bytes(0).unwrap();
    let at = |x: usize, y: usize| &bytes[(y * 258 + x) * 4..][..4];
    // Channels are swapped to BGRA.
    assert_eq!(at(1, 1), [3, 2, 1, 255]);
    assert_eq!(at(256, 256), [9, 8, 7, 255]);
    // The border repeats the nearest edge pixel, corners included.
    assert_eq!(at(0, 0), at(1, 1));
    assert_eq!(at(0, 1), at(1, 1));
    assert_eq!(at(1, 0), at(1, 1));
    assert_eq!(at(257, 257), at(256, 256));
    assert_eq!(at(100, 0), at(100, 1));
    assert_eq!(at(0, 100), at(1, 100));
    assert_eq!(at(129, 129), [30, 20, 10, 255]);
}

#[test]
fn a_sprite_image_is_scaled_to_the_device_and_carries_its_opacity() {
    let icon: std::sync::Arc<[u8]> = [200, 100, 50, 255].repeat(80 * 80).into();
    let sprite = lumilio_map_render::Sprite {
        id: "i".into(),
        width: 80,
        height: 80,
        rgba: icon,
        x: 0.,
        y: 0.,
        size: 28,
        opacity: 1.,
    };
    let full = canvas::sprite_image(&sprite, 56, 1.);
    assert_eq!((full.size(0).width.0, full.size(0).height.0), (56, 56));
    assert_eq!(&full.as_bytes(0).unwrap()[..4], [50, 100, 200, 255]);
    let faded = canvas::sprite_image(&sprite, 56, 0.7);
    assert_eq!(&faded.as_bytes(0).unwrap()[..4], [50, 100, 200, 179]);
}

/// Answers whatever the map asked for until it asks for nothing more: flat tiles,
/// and a village every `spacing` blocks.
fn answer_requests(
    view: &gpui::Entity<MapView>,
    commands: &async_channel::Receiver<Command>,
    spacing: i32,
    cx: &mut gpui::VisualTestContext,
) {
    loop {
        let mut answered = false;
        let mut events = vec![];
        while let Ok(command) = commands.try_recv() {
            answered = true;
            events.push(match command {
                Command::Tile {
                    generation,
                    request,
                    ..
                } => Event::Tile {
                    generation,
                    key: request.key,
                    result: Ok(image(60)),
                },
                Command::Objects {
                    generation,
                    key,
                    request,
                    ..
                } => {
                    let b = request.bounds;
                    let mut found = vec![];
                    let mut x = (b.min.x as i32).div_euclid(spacing) * spacing;
                    while f64::from(x) < b.max.x {
                        let mut z = (b.min.z as i32).div_euclid(spacing) * spacing;
                        while f64::from(z) < b.max.z {
                            if f64::from(x) >= b.min.x && f64::from(z) >= b.min.z {
                                found.push(icon_object(f64::from(x), f64::from(z), 0));
                            }
                            z += spacing;
                        }
                        x += spacing;
                    }
                    Event::Objects {
                        generation,
                        key,
                        result: Ok(found),
                    }
                }
                _ => continue,
            });
        }
        for event in events {
            view.update(cx, |view, cx| view.event(event, cx));
        }
        if !answered {
            break;
        }
    }
    cx.run_until_parked();
}

fn canvas_view(
    cx: &mut TestAppContext,
) -> (
    gpui::Entity<MapView>,
    &mut gpui::VisualTestContext,
    async_channel::Receiver<Command>,
) {
    let (view, cx, commands) = ready_view(cx);
    view.update(cx, |view, _| view.backend = Backend::Canvas);
    cx.simulate_resize(gpui::size(px(1000.), px(700.)));
    cx.run_until_parked();
    (view, cx, commands)
}

#[gpui::test]
fn the_canvas_backend_paints_every_visible_tile_and_icon_without_a_render_thread(
    cx: &mut TestAppContext,
) {
    let (view, cx, commands) = canvas_view(cx);
    view.read_with(cx, |view, _| {
        assert!(view.worker.is_none(), "no wgpu thread in canvas mode");
        assert!(view.image.is_none());
    });
    view.update(cx, |view, cx| {
        view.event(
            Event::Overlays {
                context: view.context.clone().unwrap(),
                layers: vec![village_layer()],
            },
            cx,
        )
    });
    answer_requests(&view, &commands, 400, cx);
    let (visible, icons, stats) = view.read_with(cx, |view, _| {
        (
            view.visible.len(),
            view.sprites().len(),
            view.stats.snapshot(),
        )
    });
    assert!(visible > 4 && icons > 10, "{visible} tiles, {icons} icons");
    assert!(stats.canvas_paints > 0, "the canvas painted");
    // Make one more frame and count exactly what it drew.
    let before = view.read_with(cx, |view, _| view.stats.snapshot());
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let drew = view.read_with(cx, |view, _| view.stats.snapshot().since(before));
    assert_eq!(drew.canvas_paints, 1);
    // `visible` carries a margin of tiles beyond the screen; the painter skips
    // those, so expect exactly the tiles whose rectangle meets the viewport.
    let on_screen = view.update(cx, |view, _| {
        let bounds = view.bounds.unwrap();
        let (w, h) = (f64::from(bounds.size.width), f64::from(bounds.size.height));
        let camera = lumilio_map_render::Camera {
            x: view.camera.x,
            z: view.camera.z,
            blocks_per_pixel: view.camera.scale,
        };
        view.scene_tiles()
            .iter()
            .filter(|tile| {
                let [l, t, r, b] = canvas::tile_edges(&camera, w, h, tile);
                !(r < 0. || b < 0. || l > w || t > h)
            })
            .count()
    });
    assert!(
        on_screen > 4 && on_screen < visible,
        "{on_screen} of {visible}"
    );
    assert_eq!(
        drew.canvas_tiles as usize, on_screen,
        "every tile on screen"
    );
    assert_eq!(drew.canvas_sprites as usize, icons, "every icon");
    assert_eq!(
        drew.canvas_images, 0,
        "images are built once, not per frame"
    );
}

#[gpui::test]
fn canvas_grid_lines_follow_the_toggles_and_the_zoom_limit(cx: &mut TestAppContext) {
    let (view, cx, _commands) = canvas_view(cx);
    let quads =
        |cx: &mut gpui::VisualTestContext| cx.update(|window, _| window.painted_quads().len());
    view.update(cx, |view, cx| {
        view.camera.x = 0.;
        view.camera.z = 0.;
        view.camera.scale = 1.;
        view.refresh();
        cx.notify();
    });
    cx.run_until_parked();
    let bare = quads(cx);
    view.update(cx, |view, cx| {
        view.layers.chunks = true;
        cx.notify();
    });
    cx.run_until_parked();
    // At 1 block per pixel the view is size blocks wide, so lines every 16.
    let (w, h) = view.read_with(cx, |view, _| {
        (f64::from(view.size[0]), f64::from(view.size[1]))
    });
    let count = |extent: f64, step: f64| ((extent / 2.) / step).floor() as usize * 2 + 1;
    assert_eq!(
        quads(cx) - bare,
        count(w, 16.) + count(h, 16.),
        "one quad per chunk boundary"
    );
    // Past 4 blocks per pixel the chunk lines are not drawn, as in the wgpu scene.
    view.update(cx, |view, cx| {
        view.camera.scale = 8.;
        view.refresh();
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(quads(cx), bare, "no chunk lines when zoomed out");
    // Region lines are every 512 blocks.
    view.update(cx, |view, cx| {
        view.layers.regions = true;
        view.camera.scale = 4.;
        view.refresh();
        cx.notify();
    });
    cx.run_until_parked();
    let regions = quads(cx) - bare;
    let (min, max) = (-(w * 4. / 2.), w * 4. / 2.);
    let across = (max / 512.).floor() - (min / 512.).ceil() + 1.;
    let (zmin, zmax) = (-(h * 4. / 2.), h * 4. / 2.);
    let down = (zmax / 512.).floor() - (zmin / 512.).ceil() + 1.;
    // Chunk lines are also on at 4 blocks per pixel, so count both.
    let chunk = |extent: f64| ((extent * 4. / 2.) / 16.).floor() as usize * 2 + 1;
    assert_eq!(regions, (across + down) as usize + chunk(w) + chunk(h));
}

#[gpui::test]
#[ignore = "measurement, prints a report"]
fn canvas_cpu_cost_per_frame_while_dragging(cx: &mut TestAppContext) {
    let (view, cx, commands) = canvas_view(cx);
    view.update(cx, |view, cx| {
        view.event(
            Event::Overlays {
                context: view.context.clone().unwrap(),
                layers: vec![village_layer()],
            },
            cx,
        )
    });
    answer_requests(&view, &commands, 400, cx);
    let (tiles, icons) = view.read_with(cx, |view, _| (view.visible.len(), view.sprites().len()));
    // Warm up so every image is built, then drag.
    for _ in 0..20 {
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
    let before = view.read_with(cx, |view, _| view.stats.snapshot());
    let started = std::time::Instant::now();
    let frames = 400;
    for _ in 0..frames {
        view.update(cx, |view, cx| {
            view.camera.pan(1., 0.5);
            view.refresh();
            cx.notify();
        });
        cx.run_until_parked();
        answer_requests(&view, &commands, 400, cx);
    }
    let total = started.elapsed();
    let report = view.read_with(cx, |view, _| view.stats.snapshot().since(before));
    println!(
        "canvas, {tiles} tiles + {icons} icons: {:.3} ms per drag frame all-in (event, layout, paint, test harness); {}",
        total.as_secs_f64() * 1e3 / f64::from(frames),
        report.line()
    );
}

#[test]
fn the_gpui_canvas_is_the_default_backend_and_wgpu_is_opt_in() {
    assert_eq!(Backend::parse(None), Backend::Canvas);
    assert_eq!(Backend::parse(Some("canvas")), Backend::Canvas);
    assert_eq!(Backend::parse(Some("")), Backend::Canvas);
    assert_eq!(Backend::parse(Some("nonsense")), Backend::Canvas);
    assert_eq!(Backend::parse(Some("wgpu")), Backend::Wgpu);
}
