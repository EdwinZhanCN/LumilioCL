//! The camera and a small rasterizer: perspective-correct texture
//! coordinates, a depth buffer, nearest sampling inside each face's texel
//! rectangle, and a light that follows the camera so the side facing the
//! viewer is always lit.

use crate::model::{Source, Triangle, V3};
use crate::{Camera, Frame, Player, Texture};

/// The middle of the player, which the camera circles.
const TARGET: V3 = V3::new(0.0, 16.0, 0.0);
/// Half the vertical field of view.
const HALF_FOV: f32 = 0.27;
/// How much of the player must fit at zoom 1, in skin pixels from the middle:
/// half the height with the hat, and half the width with spread arms.
const HALF_HEIGHT: f32 = 17.5;
const HALF_WIDTH: f32 = 11.0;
/// Nothing nearer than this is drawn; the camera never gets this close.
const NEAR: f32 = 1.0;
/// A second-layer texel with less alpha than this is a hole.
const CUTOUT: u8 = 26;
/// The plain figure's grey.
const PLAIN: [u8; 3] = [176, 176, 176];

pub(crate) struct View {
    eye: V3,
    right: V3,
    up: V3,
    forward: V3,
    focal: f32,
    half_width: f32,
    half_height: f32,
    light: V3,
}

impl View {
    pub(crate) fn new(camera: Camera, width: u32, height: u32) -> Self {
        let (half_width, half_height) = (width as f32 / 2.0, height as f32 / 2.0);
        let focal = half_height / HALF_FOV.tan();
        // Far enough that the player's height and width both fit, with room
        // for the part of the player nearer than the middle.
        let vertical = HALF_HEIGHT * focal / half_height;
        let horizontal = HALF_WIDTH * focal / half_width.max(1.0);
        let distance = (vertical.max(horizontal) + 8.0) / camera.zoom;
        let (yaw_sin, yaw_cos) = camera.yaw.sin_cos();
        let (pitch_sin, pitch_cos) = camera.pitch.sin_cos();
        let eye = TARGET
            .add(V3::new(yaw_sin * pitch_cos, pitch_sin, yaw_cos * pitch_cos).scale(distance));
        let forward = TARGET.sub(eye).normalized();
        let right = forward.cross(V3::new(0.0, 1.0, 0.0)).normalized();
        let up = right.cross(forward);
        let light = forward
            .scale(-0.8)
            .add(up.scale(0.6))
            .add(right.scale(-0.3))
            .normalized();
        Self {
            eye,
            right,
            up,
            forward,
            focal,
            half_width,
            half_height,
            light,
        }
    }

    /// Screen position (pixels, y down) and depth of a point, or `None`
    /// behind the near plane.
    pub(crate) fn project(&self, point: V3) -> Option<(f32, f32, f32)> {
        let relative = point.sub(self.eye);
        let depth = relative.dot(self.forward);
        (depth >= NEAR).then(|| {
            (
                self.half_width + relative.dot(self.right) / depth * self.focal,
                self.half_height - relative.dot(self.up) / depth * self.focal,
                depth,
            )
        })
    }
}

pub(crate) fn draw(
    triangles: &[Triangle],
    player: &Player<'_>,
    camera: Camera,
    width: u32,
    height: u32,
) -> Frame {
    let (w, h) = (width as usize, height as usize);
    let mut bgra = vec![0u8; w * h * 4];
    if w == 0 || h == 0 {
        return Frame {
            width,
            height,
            bgra,
        };
    }
    // Larger is nearer: the depth buffer keeps 1/depth.
    let mut nearest = vec![0f32; w * h];
    let view = View::new(camera, width, height);
    for triangle in triangles {
        let texture = match triangle.source {
            Source::Skin => player.skin,
            Source::Cape => player.cape,
            Source::Plain => None,
        };
        let shade = 0.55 + 0.45 * triangle.normal.dot(view.light).max(0.0);
        let Some(corners) = project(&view, triangle) else {
            continue;
        };
        fill(&corners, w, h, |x, y, inverse, u, v| {
            let at = y * w + x;
            if inverse <= nearest[at] {
                return;
            }
            let rgb = match texture {
                Some(texture) => {
                    let [r, g, b, a] = sample(texture, triangle, u, v);
                    if triangle.cutout && a < CUTOUT {
                        return;
                    }
                    [r, g, b]
                }
                None => PLAIN,
            };
            nearest[at] = inverse;
            let lit = rgb.map(|channel| (f32::from(channel) * shade).round().min(255.0) as u8);
            bgra[at * 4..at * 4 + 4].copy_from_slice(&[lit[2], lit[1], lit[0], 255]);
        });
    }
    Frame {
        width,
        height,
        bgra,
    }
}

/// A corner on the screen: position, 1/depth, and texture coordinates
/// divided by depth, ready for perspective-correct interpolation.
#[derive(Clone, Copy)]
struct Corner {
    x: f32,
    y: f32,
    inverse: f32,
    u: f32,
    v: f32,
}

fn project(view: &View, triangle: &Triangle) -> Option<[Corner; 3]> {
    let mut corners = [Corner {
        x: 0.0,
        y: 0.0,
        inverse: 0.0,
        u: 0.0,
        v: 0.0,
    }; 3];
    for (corner, (point, [u, v])) in corners
        .iter_mut()
        .zip(triangle.points.iter().zip(triangle.uv))
    {
        let (x, y, depth) = view.project(*point)?;
        let inverse = 1.0 / depth;
        *corner = Corner {
            x,
            y,
            inverse,
            u: u * inverse,
            v: v * inverse,
        };
    }
    Some(corners)
}

/// Calls `plot(x, y, 1/depth, u, v)` for every pixel whose centre the
/// triangle covers; shared edges are covered by both triangles.
fn fill(
    corners: &[Corner; 3],
    w: usize,
    h: usize,
    mut plot: impl FnMut(usize, usize, f32, f32, f32),
) {
    let [a, b, c] = corners;
    let area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    if area.abs() < f32::EPSILON {
        return;
    }
    let left = a.x.min(b.x).min(c.x).floor().max(0.0) as usize;
    let right = (a.x.max(b.x).max(c.x).ceil().max(0.0) as usize).min(w);
    let top = a.y.min(b.y).min(c.y).floor().max(0.0) as usize;
    let bottom = (a.y.max(b.y).max(c.y).ceil().max(0.0) as usize).min(h);
    let edge =
        |p: &Corner, q: &Corner, x: f32, y: f32| (q.x - p.x) * (y - p.y) - (q.y - p.y) * (x - p.x);
    for y in top..bottom {
        let py = y as f32 + 0.5;
        for x in left..right {
            let px = x as f32 + 0.5;
            let wa = edge(b, c, px, py) / area;
            let wb = edge(c, a, px, py) / area;
            let wc = edge(a, b, px, py) / area;
            if wa < 0.0 || wb < 0.0 || wc < 0.0 {
                continue;
            }
            let inverse = wa * a.inverse + wb * b.inverse + wc * c.inverse;
            let u = (wa * a.u + wb * b.u + wc * c.u) / inverse;
            let v = (wa * a.v + wb * b.v + wc * c.v) / inverse;
            plot(x, y, inverse, u, v);
        }
    }
}

/// The texel at (u, v), kept inside the face's own rectangle.
fn sample(texture: &Texture, triangle: &Triangle, u: f32, v: f32) -> [u8; 4] {
    let [u0, v0, u1, v1] = triangle.rect;
    let x = (u.floor().max(0.0) as u32).clamp(u0, u1.saturating_sub(1).max(u0));
    let y = (v.floor().max(0.0) as u32).clamp(v0, v1.saturating_sub(1).max(v0));
    let mut texel = texture.texel(x, y);
    if !triangle.cutout {
        // The first layer is opaque in the game whatever its alpha says.
        texel[3] = 255;
    }
    texel
}
