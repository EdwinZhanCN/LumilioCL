//! Focus (plan schematic-studio T31, D30): one block kind keeps its colour,
//! the rest fade or are hidden, from a mask and not a new mesh.

use lumilio_block_assets::EntityLayers;
use nucleation::UniversalSchematic;

use super::icons::{count, pack};
use crate::{Focus, Scene, SceneError, View};

#[test]
fn a_focused_kind_keeps_its_colour_and_the_rest_fade_or_hide() {
    let mut schematic = UniversalSchematic::new("focus".into());
    schematic.fill_cuboid_str((0, 0, 0), (1, 1, 1), "minecraft:stone");
    schematic.fill_cuboid_str((3, 0, 0), (4, 1, 1), "minecraft:dirt");
    let mut scene = match Scene::from_schematic(schematic, &pack(), EntityLayers::new()) {
        Ok(scene) => scene,
        Err(SceneError::NoGpu) => {
            eprintln!("skipped: no graphics adapter on this machine");
            return;
        }
        Err(error) => panic!("{error}"),
    };
    let view = View {
        background: Some([1., 1., 1., 1.]),
        ..View::new()
    };
    let draw = |scene: &mut Scene| scene.render(&view, 96, 64).unwrap();
    let all = draw(&mut scene);
    let (red, blue) = (count(&all, 2), count(&all, 0));
    assert!(
        red > 100 && blue > 100,
        "both cubes drawn: {red} red, {blue} blue"
    );

    scene.set_focus(&Focus::Fade("minecraft:stone".into()));
    let faded = draw(&mut scene);
    assert_eq!(count(&faded, 2), red, "the focused stone is untouched");
    assert_eq!(
        count(&faded, 0),
        0,
        "the dirt is washed out, no longer blue"
    );
    let washed = faded
        .bgra
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[3] == 255 && px[..3] != [255, 255, 255] && px[0] > 150 && px[1] > 150)
        .count();
    assert!(washed > 100, "but still drawn, pale: {washed}");

    // A new size makes a new renderer; the focus carries over.
    let bigger = scene.render(&view, 120, 80).unwrap();
    assert_eq!(count(&bigger, 0), 0);

    scene.set_focus(&Focus::Only("minecraft:stone".into()));
    let only = draw(&mut scene);
    // The dirt stood in front of part of the stone; hidden, it shows more.
    assert!(count(&only, 2) >= red);
    assert_eq!(count(&only, 0), 0);
    // Every pixel is the background or the red stone: no dirt, not even pale.
    let stray = only
        .bgra
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[..3] != [255, 255, 255] && !(px[2] > px[0] && px[2] > px[1]))
        .count();
    assert_eq!(stray, 0, "pixels neither background nor stone");

    scene.set_focus(&Focus::All);
    let again = draw(&mut scene);
    assert_eq!((count(&again, 2), count(&again, 0)), (red, blue));
}

fn two_cubes() -> Option<Scene> {
    let mut schematic = UniversalSchematic::new("mark".into());
    schematic.fill_cuboid_str((0, 0, 0), (1, 1, 1), "minecraft:stone");
    schematic.fill_cuboid_str((3, 0, 0), (4, 1, 1), "minecraft:dirt");
    match Scene::from_schematic(schematic, &pack(), EntityLayers::new()) {
        Ok(scene) => Some(scene),
        Err(SceneError::NoGpu) => {
            eprintln!("skipped: no graphics adapter on this machine");
            None
        }
        Err(error) => panic!("{error}"),
    }
}

/// T32: a click's block is outlined on its own, apart from any focus: only
/// the faces it shows change, near their edges, and only until it is let go.
#[test]
fn a_marked_block_is_outlined_and_nothing_else_changes() {
    let Some(mut scene) = two_cubes() else {
        return;
    };
    let view = View {
        background: Some([1., 1., 1., 1.]),
        ..View::new()
    };
    let (width, height) = (480, 320);
    let green = |frame: &crate::Frame| {
        frame
            .bgra
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|px| px[1] > 128 && px[0] < 64 && px[2] < 64)
            .count()
    };
    let plain = scene.render(&view, width, height).unwrap();
    assert_eq!(green(&plain), 0);
    let picked = scene
        .pick(&view, width, height, width as f32 / 2., height as f32 / 2.)
        .expect("a block in the middle of the picture");

    scene.set_mark(Some(picked.position), [0., 1., 0., 1.], 2.5);
    let marked = scene.render(&view, width, height).unwrap();
    let outline = green(&marked);
    assert!(outline > 20, "its edges are drawn: {outline} pixels");
    let changed = plain
        .bgra
        .as_chunks::<4>()
        .0
        .iter()
        .zip(marked.bgra.as_chunks::<4>().0)
        .filter(|(before, after)| before != after)
        .count();
    // The stone is two blocks a side: one block shows a quarter of it.
    let block = count(&plain, 2) / 4;
    assert!(
        changed < block * 2 / 3,
        "a band inside one block, not the block: {changed} of about {block}"
    );

    // Where a card floats: the block's box on screen holds the clicked
    // pixel and is about a block's size.
    let [left, top, right, bottom] = scene.mark_rect(&view, width, height).unwrap();
    assert!(
        left < 0.5 && 0.5 < right && top < 0.5 && 0.5 < bottom,
        "{:?}",
        [left, top, right, bottom]
    );
    assert!(
        right - left < 0.3 && bottom - top < 0.3,
        "one block, not the build"
    );

    // A new size makes a new renderer; the mark carries over.
    assert!(green(&scene.render(&view, 240, 160).unwrap()) > 0);
    scene.set_mark(None, [0., 1., 0., 1.], 2.5);
    let cleared = scene.render(&view, width, height).unwrap();
    assert_eq!(cleared.bgra, plain.bgra, "let go, the picture is as it was");
    assert_eq!(scene.mark_rect(&view, width, height), None);
}

/// The outline follows the block's own geometry: a lower half block's box
/// ends halfway up its cell, and the face its full neighbour turns to it
/// belongs to the neighbour, not to it.
#[test]
fn an_outline_follows_the_shape_of_its_block() {
    let mut schematic = UniversalSchematic::new("shape".into());
    schematic.set_block_str(0, 0, 0, "minecraft:dirt_slab");
    schematic.set_block_str(1, 0, 0, "minecraft:stone");
    let scene = match Scene::from_schematic(schematic, &pack(), EntityLayers::new()) {
        Ok(scene) => scene,
        Err(SceneError::NoGpu) => {
            eprintln!("skipped: no graphics adapter on this machine");
            return;
        }
        Err(error) => panic!("{error}"),
    };
    assert_eq!(
        scene.shape([0, 0, 0]),
        ([-0.5, -0.5, -0.5], [0.5, 0.0, 0.5])
    );
    assert_eq!(scene.shape([1, 0, 0]), ([0.5, -0.5, -0.5], [1.5, 0.5, 0.5]));
    assert_eq!(
        scene.shape([3, 0, 0]),
        ([2.5, -0.5, -0.5], [3.5, 0.5, 0.5]),
        "an empty cell is its own box"
    );
}

/// With 只看这种 the hidden blocks are not there to click: a pick passes
/// through them to the kind that is drawn.
#[test]
fn a_pick_passes_through_blocks_the_focus_hides() {
    let Some(mut scene) = two_cubes() else {
        return;
    };
    let view = View::new();
    let (width, height) = (96, 64);
    let cells: Vec<(f32, f32)> = (0..height)
        .step_by(2)
        .flat_map(|y| (0..width).step_by(2).map(move |x| (x as f32, y as f32)))
        .collect();
    let pick = |scene: &Scene, (x, y): (f32, f32)| {
        scene
            .pick(&view, width, height, x, y)
            .map(|picked| picked.state)
    };
    let over_dirt: Vec<(f32, f32)> = cells
        .iter()
        .copied()
        .filter(|&at| pick(&scene, at).as_deref() == Some("minecraft:dirt"))
        .collect();
    assert!(!over_dirt.is_empty(), "the dirt is in the picture");

    scene.set_focus(&Focus::Only("minecraft:stone".into()));
    let behind: Vec<Option<String>> = over_dirt.iter().map(|&at| pick(&scene, at)).collect();
    assert!(
        behind
            .iter()
            .all(|state| state.as_deref() != Some("minecraft:dirt")),
        "hidden dirt is never picked"
    );
    assert!(
        behind
            .iter()
            .any(|state| state.as_deref() == Some("minecraft:stone")),
        "the stone it stood in front of is"
    );
    scene.set_focus(&Focus::Fade("minecraft:stone".into()));
    assert_eq!(
        pick(&scene, over_dirt[0]).as_deref(),
        Some("minecraft:dirt"),
        "a faded block is still drawn, so still picked"
    );
}

/// T25: a slice draws its layer and those below; the cut shows the layer's
/// own tops, which the full mesh hides under the layer above, and a click
/// lands on the layer, not above it. Without the slice the picture is whole.
#[test]
fn a_slice_shows_its_layer_from_above_and_clicks_land_on_it() {
    use crate::{Angle, Slice, SliceAxis};
    let mut schematic = UniversalSchematic::new("slice".into());
    schematic.fill_cuboid_str((0, 0, 0), (1, 2, 1), "minecraft:stone");
    // Faces that name the side they hide against, as the game's do: the
    // mesher culls them between neighbours.
    let pack = super::tiny_pack();
    let mut scene = match Scene::from_schematic(schematic, &pack, EntityLayers::new()) {
        Ok(scene) => scene,
        Err(SceneError::NoGpu) => {
            eprintln!("skipped: no graphics adapter on this machine");
            return;
        }
        Err(error) => panic!("{error}"),
    };
    let top = View {
        background: Some([1., 1., 1., 1.]),
        ..View::new()
    }
    .at(Angle::Top);
    let (width, height) = (128, 128);
    let whole = scene.render(&top, width, height).unwrap();
    let red = count(&whole, 2);
    assert!(red > 500, "the stone from above: {red}");
    assert_eq!(
        scene.pick(&top, width, height, 64., 64.).unwrap().position[1],
        2
    );

    scene
        .set_slice(Some(Slice {
            axis: SliceAxis::Y,
            layer: 1,
        }))
        .unwrap();
    let cut = scene.render(&top, width, height).unwrap();
    assert!(
        count(&cut, 2) > red / 2,
        "the middle layer's tops show: {} of {red}",
        count(&cut, 2)
    );
    assert_eq!(
        scene.pick(&top, width, height, 64., 64.).unwrap().position[1],
        1,
        "the click lands on the cut layer"
    );

    scene.set_slice(None).unwrap();
    let again = scene.render(&top, width, height).unwrap();
    assert_eq!(count(&again, 2), red, "whole again");
}

/// T25: a selected box is drawn with a band of its colour along its edges;
/// without one there is none of that colour.
#[test]
fn a_selection_draws_a_band_around_its_box() {
    use crate::Angle;
    let mut schematic = UniversalSchematic::new("selection".into());
    schematic.fill_cuboid_str((0, 0, 0), (3, 0, 3), "minecraft:stone");
    let pack = super::tiny_pack();
    let mut scene = match Scene::from_schematic(schematic, &pack, EntityLayers::new()) {
        Ok(scene) => scene,
        Err(SceneError::NoGpu) => {
            eprintln!("skipped: no graphics adapter on this machine");
            return;
        }
        Err(error) => panic!("{error}"),
    };
    let top = View {
        background: Some([1., 1., 1., 1.]),
        ..View::new()
    }
    .at(Angle::Top);
    let green = |frame: &crate::Frame| {
        frame
            .bgra
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|px| px[1] > 200 && px[0] < 64 && px[2] < 64)
            .count()
    };
    let plain = scene.render(&top, 128, 128).unwrap();
    assert_eq!(green(&plain), 0);
    scene.set_selection(Some(([1, 0, 1], [2, 0, 2])), [0., 1., 0., 1.]);
    let selected = scene.render(&top, 128, 128).unwrap();
    assert!(green(&selected) > 40, "the band: {}", green(&selected));
    scene.set_selection(None, [0., 1., 0., 1.]);
    assert_eq!(green(&scene.render(&top, 128, 128).unwrap()), 0, "cleared");
}
