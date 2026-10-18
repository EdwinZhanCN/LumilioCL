//! Per-frame culling of entity instances (LumilioCL local change, plan
//! schematic-studio T8; see `forks/README.md`).
//!
//! Each instance is a model's bounding sphere moved by its transform. Those
//! wholly outside the view frustum, or farther from the eye than the distance
//! limit, are left out of the frame's instance buffer.

use crate::meshing::{transform_point, EntityInstance, EntityInstances, Transform};

/// The planes of a view frustum, `a·x + b·y + c·z + d >= 0` inside, each
/// normalised so the left side is a distance.
#[derive(Clone, Debug)]
pub struct Frustum {
    planes: Vec<[f32; 4]>,
}

impl Frustum {
    /// From a column-major view-projection with wgpu clip space
    /// (`-w <= x, y <= w`, `0 <= z <= w`), forward or reversed depth.
    /// A plane with no direction (the far plane at infinity of a reversed
    /// perspective) bounds nothing and is dropped.
    pub fn from_view_proj(m: &[[f32; 4]; 4]) -> Self {
        let row = |r: usize| [m[0][r], m[1][r], m[2][r], m[3][r]];
        let [x, y, z, w] = [row(0), row(1), row(2), row(3)];
        let add = |a: [f32; 4], b: [f32; 4], sign: f32| -> [f32; 4] {
            [0, 1, 2, 3].map(|i| a[i] + sign * b[i])
        };
        let planes = [
            add(w, x, 1.0),
            add(w, x, -1.0),
            add(w, y, 1.0),
            add(w, y, -1.0),
            z,
            add(w, z, -1.0),
        ]
        .into_iter()
        .filter_map(|p| {
            let length = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            (length > 1e-6).then(|| p.map(|v| v / length))
        })
        .collect();
        Self { planes }
    }

    /// Whether any part of the sphere can be inside.
    pub fn sees(&self, center: [f32; 3], radius: f32) -> bool {
        self.planes
            .iter()
            .all(|p| p[0] * center[0] + p[1] * center[1] + p[2] * center[2] + p[3] >= -radius)
    }
}

/// What a frame keeps: inside `frustum`, and no farther than `max_distance`
/// from `eye` when there is a limit. `back` points from the scene toward the
/// camera, which camera-facing instances turn to (T35).
pub struct Culling<'a> {
    pub frustum: &'a Frustum,
    pub eye: [f32; 3],
    pub max_distance: Option<f32>,
    pub back: [f32; 3],
}

impl Culling<'_> {
    pub fn keeps(&self, center: [f32; 3], radius: f32) -> bool {
        if let Some(limit) = self.max_distance {
            let d = [0, 1, 2].map(|i| center[i] - self.eye[i]);
            let distance = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if distance - radius > limit {
                return false;
            }
        }
        self.frustum.sees(center, radius)
    }
}

/// A model's bounding sphere in model space: centre and radius.
pub type Sphere = ([f32; 3], f32);

/// The bounding spheres of `entities`' models, by model index.
pub fn spheres(entities: &EntityInstances) -> Vec<Sphere> {
    entities
        .models
        .iter()
        .map(|model| (model.center, model.radius))
        .collect()
}

/// Instances of each model that the frame keeps, farthest first (the order
/// translucent parts blend correctly in), by model index.
pub fn visible(
    spheres: &[Sphere],
    instances: &[EntityInstance],
    culling: &Culling,
) -> Vec<Vec<Transform>> {
    let mut kept: Vec<Vec<(f32, Transform)>> = vec![Vec::new(); spheres.len()];
    for instance in instances {
        let (center, radius) = spheres[instance.model];
        let transform = instance.billboard.map_or(instance.transform, |billboard| {
            billboard.turned(culling.back)
        });
        let center = transform_point(&transform, center);
        if !culling.keeps(center, radius) {
            continue;
        }
        let d = [0, 1, 2].map(|i| center[i] - culling.eye[i]);
        kept[instance.model].push((d[0] * d[0] + d[1] * d[1] + d[2] * d[2], transform));
    }
    kept.into_iter()
        .map(|mut model| {
            model.sort_by(|a, b| b.0.total_cmp(&a.0));
            model.into_iter().map(|(_, t)| t).collect()
        })
        .collect()
}

/// The unit vector from the scene toward the camera of a view: back along
/// the line through the centre of the near and far planes.
pub fn back(inv_view_proj: &[[f32; 4]; 4], reversed_depth: bool) -> [f32; 3] {
    let m = inv_view_proj;
    let unproject = |z: f32| {
        let p = [0.0, 0.0, z, 1.0];
        let v = [0, 1, 2, 3].map(|r| (0..4).map(|c| m[c][r] * p[c]).sum::<f32>());
        let w = if v[3].abs() < 1e-9 { 1.0 } else { v[3] };
        [v[0] / w, v[1] / w, v[2] / w]
    };
    let (near, toward) = if reversed_depth {
        (1.0, 0.5)
    } else {
        (0.0, 0.5)
    };
    let (a, b) = (unproject(near), unproject(toward));
    let d = [0, 1, 2].map(|i| a[i] - b[i]);
    let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if length < 1e-9 {
        return [0.0, 0.0, 1.0];
    }
    d.map(|c| c / length)
}

/// The eye of a view: the given first-person position, else the point the
/// inverse view-projection puts at the centre of the near plane (the orbit
/// camera's near plane is 1% of its distance from the eye).
pub fn eye(
    position: Option<[f32; 3]>,
    inv_view_proj: &[[f32; 4]; 4],
    reversed_depth: bool,
) -> [f32; 3] {
    if let Some(position) = position {
        return position;
    }
    let near = if reversed_depth { 1.0 } else { 0.0 };
    let p = [0.0, 0.0, near, 1.0];
    let m = inv_view_proj;
    let v = [0, 1, 2, 3].map(|r| (0..4).map(|c| m[c][r] * p[c]).sum::<f32>());
    if v[3].abs() < 1e-9 {
        return [v[0], v[1], v[2]];
    }
    [v[0] / v[3], v[1] / v[3], v[2] / v[3]]
}

#[cfg(test)]
mod tests;
