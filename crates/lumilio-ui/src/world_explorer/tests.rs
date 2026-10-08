use super::camera::Camera;
use super::*;
use gpui::{Modifiers, TestAppContext, point};
use lumilio_plugin_api::map::{BaseMapInfo, WorldId};
#[test]
fn zoom_keeps_pointer_world_coordinate_and_lod_uses_fourfold_steps() {
    let mut camera = Camera::default();
    let before = camera.world([100., 50.], [800, 600]);
    camera.zoom(0.5, [100., 50.], [800, 600]);
    assert_eq!(camera.world([100., 50.], [800, 600]), before);
    assert_eq!(camera.level(), 0);
    camera.scale = 16.;
    assert_eq!(camera.level(), 2);
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
