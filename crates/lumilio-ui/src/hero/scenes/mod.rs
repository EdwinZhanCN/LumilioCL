//! The hero scenes. The four showcase scenes each turn one game mechanic into
//! motion; Hearth is Continue's own scene.

pub mod caves;
pub mod dawn;
pub mod hearth;
pub mod portal;
pub mod redstone;

pub use hearth::Landmark;

use super::raster::PixelGrid;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Scene {
    Dawn,
    Caves,
    Redstone,
    Portal,
    /// Continue's night camp, with the instance's world as a landmark.
    Hearth(Landmark),
}

impl Scene {
    /// The showcase carousel, in order. Hearth belongs to Continue only.
    pub const ALL: [Self; 4] = [Self::Dawn, Self::Caves, Self::Redstone, Self::Portal];

    /// Renders the scene. `flare` (0–1) is the pointer resting on Continue;
    /// only Hearth answers it.
    pub fn render(self, grid: &mut PixelGrid, age: f32, flare: f32) {
        match self {
            Self::Dawn => dawn::render(grid, age),
            Self::Caves => caves::render(grid, age),
            Self::Redstone => redstone::render(grid, age),
            Self::Portal => portal::render(grid, age),
            Self::Hearth(landmark) => hearth::render(grid, age, landmark, flare),
        }
    }

    /// A composed moment used when motion is reduced.
    pub const fn still_age(self) -> f32 {
        match self {
            Self::Dawn => 7.2,
            Self::Caves => 9.,
            Self::Redstone => 3.4,
            Self::Portal => 4.6,
            Self::Hearth(_) => 3.,
        }
    }

    pub fn eyebrow(self) -> &'static str {
        match self {
            Self::Dawn => crate::tr!("hero-dawn-eyebrow"),
            Self::Caves => crate::tr!("hero-caves-eyebrow"),
            Self::Redstone => crate::tr!("hero-redstone-eyebrow"),
            Self::Portal => crate::tr!("hero-portal-eyebrow"),
            Self::Hearth(_) => crate::tr!("hero-hearth-eyebrow"),
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Dawn => crate::tr!("hero-dawn-title"),
            Self::Caves => crate::tr!("hero-caves-title"),
            Self::Redstone => crate::tr!("hero-redstone-title"),
            Self::Portal => crate::tr!("hero-portal-title"),
            Self::Hearth(_) => crate::tr!("hero-hearth-title"),
        }
    }

    pub fn caption(self) -> &'static str {
        match self {
            Self::Dawn => crate::tr!("hero-dawn-caption"),
            Self::Caves => crate::tr!("hero-caves-caption"),
            Self::Redstone => crate::tr!("hero-redstone-caption"),
            Self::Portal => crate::tr!("hero-portal-caption"),
            Self::Hearth(_) => crate::tr!("hero-hearth-caption"),
        }
    }

    /// A live readout that tracks what the scene is doing right now. `size`
    /// is the texel grid the scene was last rasterised at.
    pub fn hud(self, age: f32, size: (i32, i32)) -> String {
        match self {
            Self::Dawn => crate::tr!("hero-dawn-hud", time = dawn::clock(age)),
            Self::Caves => {
                let cave = caves::Cave::new(size.0, size.1);
                crate::tr!("hero-caves-hud", torches = cave.torches_lit(age))
            }
            Self::Redstone => crate::tr!("hero-redstone-hud", signal = redstone::signal(age)),
            Self::Portal => {
                if portal::charge(age) > 0.99 {
                    crate::tr!("hero-portal-hud-active").to_owned()
                } else if age >= portal::IGNITE_AT {
                    crate::tr!("hero-portal-hud-igniting").to_owned()
                } else {
                    crate::tr!("hero-portal-hud-inactive").to_owned()
                }
            }
            Self::Hearth(_) => crate::tr!("hero-hearth-hud", light = hearth::FIRE_LEVEL),
        }
    }
}

#[cfg(test)]
mod tests {
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
                            assert!(
                                texel.r.is_finite() && texel.g.is_finite() && texel.b.is_finite()
                            );
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
}
