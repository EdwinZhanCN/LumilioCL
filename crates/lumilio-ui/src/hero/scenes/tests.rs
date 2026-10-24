use super::Scene;
use crate::hero::raster::PixelGrid;

#[test]
fn every_scene_fills_every_texel_at_many_sizes_and_ages() {
    let hearths = [
        super::Landmark::Village,
        super::Landmark::Portal,
        super::Landmark::Mine,
        super::Landmark::Lamp,
    ]
    .map(Scene::Hearth);
    for scene in Scene::ALL.into_iter().chain(hearths) {
        for (w, h) in [(48, 20), (170, 72), (190, 104)] {
            for age in [0., 1.4, scene.still_age(), 12.] {
                let mut grid = PixelGrid::new(w, h);
                scene.render(&mut grid, age, 1.);
                // Scenes may never leave the NaN-free, finite colour space.
                for y in 0..h {
                    for x in 0..w {
                        let texel = grid.get(x, y);
                        assert!(texel.r.is_finite() && texel.g.is_finite() && texel.b.is_finite());
                    }
                }
            }
        }
    }
}

#[test]
fn huds_follow_the_scene_clock() {
    const SIZE: (i32, i32) = (170, 72);
    assert_ne!(Scene::Dawn.hud(0., SIZE), Scene::Dawn.hud(9., SIZE));
    assert_ne!(Scene::Caves.hud(0., SIZE), Scene::Caves.hud(9., SIZE));
    assert_eq!(Scene::Redstone.hud(0., SIZE), "信号强度 0");
    assert_eq!(Scene::Redstone.hud(2., SIZE), "信号强度 15");
    assert_eq!(Scene::Portal.hud(0., SIZE), "传送门 未激活");
    assert_eq!(Scene::Portal.hud(6., SIZE), "传送门 已激活");
}
