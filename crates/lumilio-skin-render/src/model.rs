//! The player as triangles. Units are skin pixels; the feet stand at y 0, the
//! player faces +z and their right hand is on −x. Boxes and texture offsets
//! follow the game's player model; each box's six faces take their pictures
//! from the usual unwrap at its offset:
//!
//! ```text
//!            u  u+d     u+d+w   u+d+2w
//!         v  ·  [ top ] [bottom]
//!       v+d  [right][front][left][ back ]
//!  v+d+h
//! ```

use crate::{Arms, Player};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct V3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl V3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    pub fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
    pub fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
    pub fn scale(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
    pub fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: Self) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn normalized(self) -> Self {
        let length = self.dot(self).sqrt();
        if length > 0.0 {
            self.scale(1.0 / length)
        } else {
            self
        }
    }
}

/// Where a part sits: a turn about y, a roll about z, a tilt about x, in that
/// order, then a move.
#[derive(Clone, Copy, Debug)]
struct Place {
    turn_y: f32,
    tilt_x: f32,
    roll_z: f32,
    at: V3,
}

impl Place {
    const fn at(x: f32, y: f32, z: f32) -> Self {
        Self {
            turn_y: 0.0,
            tilt_x: 0.0,
            roll_z: 0.0,
            at: V3::new(x, y, z),
        }
    }

    fn rotate(&self, p: V3) -> V3 {
        let (s, c) = self.turn_y.sin_cos();
        let p = V3::new(p.x * c + p.z * s, p.y, -p.x * s + p.z * c);
        let (s, c) = self.roll_z.sin_cos();
        let p = V3::new(p.x * c - p.y * s, p.x * s + p.y * c, p.z);
        let (s, c) = self.tilt_x.sin_cos();
        V3::new(p.x, p.y * c - p.z * s, p.y * s + p.z * c)
    }

    fn apply(&self, p: V3) -> V3 {
        self.rotate(p).add(self.at)
    }
}

/// Which picture a triangle takes its colour from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Source {
    Skin,
    Cape,
    /// No picture: plain grey.
    Plain,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Triangle {
    pub points: [V3; 3],
    /// Texture coordinates in texels of the source picture.
    pub uv: [[f32; 2]; 3],
    /// The face's texel rectangle `[u0, v0, u1, v1)`; samples stay inside it
    /// so a face never bleeds its neighbour's pixels.
    pub rect: [u32; 4],
    pub normal: V3,
    pub source: Source,
    /// Second-layer faces drop transparent texels; first-layer faces are
    /// opaque, as in the game.
    pub cutout: bool,
}

/// A box's size in texels and where its unwrap starts.
#[derive(Clone, Copy)]
struct Unwrap {
    w: f32,
    h: f32,
    d: f32,
    u: f32,
    v: f32,
}

/// Inflation of the second layer around the first: the hat by half a pixel,
/// the rest by a quarter, as the game draws it.
const HAT: f32 = 0.5;
const CLOTHES: f32 = 0.25;
/// Arms hang a little away from the body.
const ARM_SPREAD: f32 = 0.08;
/// The cape hangs a little away from the back.
const CAPE_TILT: f32 = 0.14;

pub(crate) fn triangles(player: &Player<'_>) -> Vec<Triangle> {
    let skin = if player.skin.is_some() {
        Source::Skin
    } else {
        Source::Plain
    };
    let scale = player.skin.map_or(1.0, |skin| skin.width as f32 / 64.0);
    let arm = match player.arms {
        Arms::Classic => 4.0,
        Arms::Slim => 3.0,
    };
    let mut out = Vec::new();
    let mut part = |unwrap: Unwrap, place: Place, origin: V3, inflate: f32, outer: bool| {
        if outer && (!player.outer_layer || skin == Source::Plain) {
            return;
        }
        boxed(&mut out, unwrap, place, origin, inflate, skin, scale, outer);
    };
    let uw = |w: f32, h: f32, d: f32, u: f32, v: f32| Unwrap { w, h, d, u, v };

    // Head: 8×8×8 above the body.
    let head = Place::at(0.0, 24.0, 0.0);
    let head_origin = V3::new(-4.0, 0.0, -4.0);
    part(uw(8.0, 8.0, 8.0, 0.0, 0.0), head, head_origin, 0.0, false);
    part(uw(8.0, 8.0, 8.0, 32.0, 0.0), head, head_origin, HAT, true);

    // Body: 8×12×4.
    let body = Place::at(0.0, 12.0, 0.0);
    let body_origin = V3::new(-4.0, 0.0, -2.0);
    part(
        uw(8.0, 12.0, 4.0, 16.0, 16.0),
        body,
        body_origin,
        0.0,
        false,
    );
    part(
        uw(8.0, 12.0, 4.0, 16.0, 32.0),
        body,
        body_origin,
        CLOTHES,
        true,
    );

    // Arms hang from the shoulders, 2 pixels below the top of the body.
    let right_arm = Place {
        roll_z: -ARM_SPREAD,
        ..Place::at(-4.0, 22.0, 0.0)
    };
    let right_origin = V3::new(-arm, -10.0, -2.0);
    part(
        uw(arm, 12.0, 4.0, 40.0, 16.0),
        right_arm,
        right_origin,
        0.0,
        false,
    );
    part(
        uw(arm, 12.0, 4.0, 40.0, 32.0),
        right_arm,
        right_origin,
        CLOTHES,
        true,
    );
    let left_arm = Place {
        roll_z: ARM_SPREAD,
        ..Place::at(4.0, 22.0, 0.0)
    };
    let left_origin = V3::new(0.0, -10.0, -2.0);
    part(
        uw(arm, 12.0, 4.0, 32.0, 48.0),
        left_arm,
        left_origin,
        0.0,
        false,
    );
    part(
        uw(arm, 12.0, 4.0, 48.0, 48.0),
        left_arm,
        left_origin,
        CLOTHES,
        true,
    );

    // Legs: 4×12×4 under the body.
    let right_leg = Place::at(-4.0, 0.0, -2.0);
    let left_leg = Place::at(0.0, 0.0, -2.0);
    let leg_origin = V3::new(0.0, 0.0, 0.0);
    part(
        uw(4.0, 12.0, 4.0, 0.0, 16.0),
        right_leg,
        leg_origin,
        0.0,
        false,
    );
    part(
        uw(4.0, 12.0, 4.0, 0.0, 32.0),
        right_leg,
        leg_origin,
        CLOTHES,
        true,
    );
    part(
        uw(4.0, 12.0, 4.0, 16.0, 48.0),
        left_leg,
        leg_origin,
        0.0,
        false,
    );
    part(
        uw(4.0, 12.0, 4.0, 0.0, 48.0),
        left_leg,
        leg_origin,
        CLOTHES,
        true,
    );

    if let Some(cape) = player.cape {
        // 10×16×1 from the shoulders, turned to face backwards (its outer
        // picture is the unwrap's front) and swung a little off the back.
        let place = Place {
            turn_y: std::f32::consts::PI,
            tilt_x: CAPE_TILT,
            roll_z: 0.0,
            // Just behind the jacket, so the two never fight for a pixel.
            at: V3::new(0.0, 24.0, -2.4),
        };
        boxed(
            &mut out,
            uw(10.0, 16.0, 1.0, 0.0, 0.0),
            place,
            V3::new(-5.0, -16.0, 0.0),
            0.0,
            Source::Cape,
            cape.width as f32 / 64.0,
            false,
        );
    }
    out
}

/// The six faces of a box whose near-lower-right corner is `origin` in the
/// part's space, as two triangles each.
#[allow(clippy::too_many_arguments)]
fn boxed(
    out: &mut Vec<Triangle>,
    unwrap: Unwrap,
    place: Place,
    origin: V3,
    inflate: f32,
    source: Source,
    scale: f32,
    cutout: bool,
) {
    let Unwrap { w, h, d, u, v } = unwrap;
    let (x0, y0, z0) = (origin.x - inflate, origin.y - inflate, origin.z - inflate);
    let (x1, y1, z1) = (
        origin.x + w + inflate,
        origin.y + h + inflate,
        origin.z + d + inflate,
    );
    let p = |x: f32, y: f32, z: f32| V3::new(x, y, z);
    // Each face: four corners (top-left, top-right, bottom-right,
    // bottom-left as seen from outside) with their texture coordinates.
    let faces: [[(V3, [f32; 2]); 4]; 6] = [
        // Front (+z): left to right is −x to +x.
        [
            (p(x0, y1, z1), [u + d, v + d]),
            (p(x1, y1, z1), [u + d + w, v + d]),
            (p(x1, y0, z1), [u + d + w, v + d + h]),
            (p(x0, y0, z1), [u + d, v + d + h]),
        ],
        // Back (−z): seen from behind, left to right is +x to −x.
        [
            (p(x1, y1, z0), [u + 2.0 * d + w, v + d]),
            (p(x0, y1, z0), [u + 2.0 * d + 2.0 * w, v + d]),
            (p(x0, y0, z0), [u + 2.0 * d + 2.0 * w, v + d + h]),
            (p(x1, y0, z0), [u + 2.0 * d + w, v + d + h]),
        ],
        // Right (−x): left to right is back to front.
        [
            (p(x0, y1, z0), [u, v + d]),
            (p(x0, y1, z1), [u + d, v + d]),
            (p(x0, y0, z1), [u + d, v + d + h]),
            (p(x0, y0, z0), [u, v + d + h]),
        ],
        // Left (+x): left to right is front to back.
        [
            (p(x1, y1, z1), [u + d + w, v + d]),
            (p(x1, y1, z0), [u + 2.0 * d + w, v + d]),
            (p(x1, y0, z0), [u + 2.0 * d + w, v + d + h]),
            (p(x1, y0, z1), [u + d + w, v + d + h]),
        ],
        // Top (+y): the back edge is the picture's top row.
        [
            (p(x0, y1, z0), [u + d, v]),
            (p(x1, y1, z0), [u + d + w, v]),
            (p(x1, y1, z1), [u + d + w, v + d]),
            (p(x0, y1, z1), [u + d, v + d]),
        ],
        // Bottom (−y): the front edge is the picture's top row.
        [
            (p(x0, y0, z1), [u + d + w, v]),
            (p(x1, y0, z1), [u + d + 2.0 * w, v]),
            (p(x1, y0, z0), [u + d + 2.0 * w, v + d]),
            (p(x0, y0, z0), [u + d + w, v + d]),
        ],
    ];
    for face in faces {
        let corners = face.map(|(point, uv)| (place.apply(point), [uv[0] * scale, uv[1] * scale]));
        let (min_u, max_u, min_v, max_v) = corners.iter().fold(
            (f32::MAX, f32::MIN, f32::MAX, f32::MIN),
            |(a, b, c, e), (_, [cu, cv])| (a.min(*cu), b.max(*cu), c.min(*cv), e.max(*cv)),
        );
        let rect = [
            min_u.round() as u32,
            min_v.round() as u32,
            max_u.round() as u32,
            max_v.round() as u32,
        ];
        // Outward: down the face's left edge, crossed with along its top.
        let normal = corners[3]
            .0
            .sub(corners[0].0)
            .cross(corners[1].0.sub(corners[0].0))
            .normalized();
        for [a, b, c] in [[0, 1, 2], [0, 2, 3]] {
            out.push(Triangle {
                points: [corners[a].0, corners[b].0, corners[c].0],
                uv: [corners[a].1, corners[b].1, corners[c].1],
                rect,
                normal,
                source,
                cutout,
            });
        }
    }
}
