//! Picking a block under the pointer (plan schematic-studio T10): the pixel
//! becomes a ray through the same camera the renderer uses, and a voxel walk
//! (Amanatides and Woo's DDA) over the schematic grid returns the first cell
//! that holds a block. Pure functions; the scene supplies the grid.

use nucleation::rendering::CameraConfig;
use nucleation::rendering::camera::compute_view_proj_reversed;

/// A ray in grid space, where block `n` fills `n ..= n + 1` on each axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub origin: [f32; 3],
    /// Need not be normalised; must not be zero.
    pub direction: [f32; 3],
}

/// The block cells a walk may visit, both corners inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
    pub min: [i32; 3],
    pub max: [i32; 3],
}

/// Walks `ray` cell by cell through `grid` and returns the first cell for which `solid` is true.
/// A ray that misses the grid, or crosses it through air only, finds nothing.
pub fn march(ray: Ray, grid: Grid, mut solid: impl FnMut([i32; 3]) -> bool) -> Option<[i32; 3]> {
    if (0..3).any(|axis| grid.min[axis] > grid.max[axis]) {
        return None;
    }
    let low = grid.min.map(|v| v as f32);
    let high = grid.max.map(|v| v as f32 + 1.0);
    // Slab test: where the ray enters and leaves the grid's box.
    let (mut enter, mut leave) = (0.0f32, f32::INFINITY);
    for axis in 0..3 {
        let (o, d) = (ray.origin[axis], ray.direction[axis]);
        if d.abs() < 1e-12 {
            if o < low[axis] || o > high[axis] {
                return None;
            }
            continue;
        }
        let (a, b) = ((low[axis] - o) / d, (high[axis] - o) / d);
        enter = enter.max(a.min(b));
        leave = leave.min(a.max(b));
    }
    if enter > leave || !enter.is_finite() {
        return None;
    }
    let start = [0, 1, 2].map(|axis| ray.origin[axis] + ray.direction[axis] * enter);
    let mut cell = [0, 1, 2].map(|axis| {
        (start[axis].floor() as i32)
            // On the far face of the box `floor` lands one past the last cell.
            .clamp(grid.min[axis], grid.max[axis])
    });
    let step = ray.direction.map(|d| if d > 0.0 { 1 } else { -1 });
    let mut next = [0.0f32; 3];
    let mut delta = [f32::INFINITY; 3];
    for axis in 0..3 {
        let d = ray.direction[axis];
        if d.abs() < 1e-12 {
            next[axis] = f32::INFINITY;
            continue;
        }
        let boundary = cell[axis] as f32 + if d > 0.0 { 1.0 } else { 0.0 };
        next[axis] = (boundary - ray.origin[axis]) / d;
        delta[axis] = 1.0 / d.abs();
    }
    loop {
        if (0..3).any(|axis| cell[axis] < grid.min[axis] || cell[axis] > grid.max[axis]) {
            return None;
        }
        if solid(cell) {
            return Some(cell);
        }
        let axis = if next[0] <= next[1] && next[0] <= next[2] {
            0
        } else if next[1] <= next[2] {
            1
        } else {
            2
        };
        if next[axis] > leave + 1e-4 {
            return None;
        }
        cell[axis] += step[axis];
        next[axis] += delta[axis];
    }
}

/// Where `point`, in mesh space, lands in a `width` × `height` frame drawn with `camera` around
/// `bounds` (the inverse of [`camera_ray`], plan schematic-studio T32): pixels from the top left,
/// or `None` behind the eye.
pub fn project(
    camera: &CameraConfig,
    bounds: ([f32; 3], [f32; 3]),
    width: u32,
    height: u32,
    point: [f32; 3],
) -> Option<[f32; 2]> {
    let (width, height) = (width.max(1) as f32, height.max(1) as f32);
    let (view_proj, _) = compute_view_proj_reversed(bounds.0, bounds.1, width / height, camera);
    let v = [point[0], point[1], point[2], 1.0];
    let mut clip = [0.0f32; 4];
    for (row, value) in clip.iter_mut().enumerate() {
        *value = (0..4).map(|col| view_proj[col][row] * v[col]).sum();
    }
    if clip[3] <= 1e-6 {
        return None;
    }
    let (x, y) = (clip[0] / clip[3], clip[1] / clip[3]);
    Some([(x + 1.0) / 2.0 * width, (1.0 - y) / 2.0 * height])
}

/// The ray through pixel `(x, y)` (top-left origin) of a `width` × `height` frame drawn with
/// `camera` around a model whose mesh spans `bounds_min ..= bounds_max`, in grid space.
///
/// The mesher puts block `n` at `n - 0.5 ..= n + 0.5`, so the mesh-space ray is shifted by half
/// a block. The two points unprojected are the near plane and a point between it and infinity,
/// as the renderer's sky pass does with its reversed depth.
pub fn camera_ray(
    camera: &CameraConfig,
    bounds: ([f32; 3], [f32; 3]),
    width: u32,
    height: u32,
    x: f32,
    y: f32,
) -> Option<Ray> {
    let (width, height) = (width.max(1) as f32, height.max(1) as f32);
    let (_, inverse) = compute_view_proj_reversed(bounds.0, bounds.1, width / height, camera);
    let ndc_x = x / width * 2.0 - 1.0;
    let ndc_y = 1.0 - y / height * 2.0;
    let unproject = |depth: f32| {
        let v = [ndc_x, ndc_y, depth, 1.0];
        let mut out = [0.0f32; 4];
        for (row, value) in out.iter_mut().enumerate() {
            *value = (0..4).map(|col| inverse[col][row] * v[col]).sum();
        }
        (out[3].abs() > 1e-12).then(|| [out[0] / out[3], out[1] / out[3], out[2] / out[3]])
    };
    let near = unproject(1.0)?;
    let far = unproject(0.5)?;
    let direction = [far[0] - near[0], far[1] - near[1], far[2] - near[2]];
    if direction.iter().all(|d| d.abs() < 1e-12) || direction.iter().any(|d| !d.is_finite()) {
        return None;
    }
    Some(Ray {
        origin: near.map(|v| v + 0.5),
        direction,
    })
}
