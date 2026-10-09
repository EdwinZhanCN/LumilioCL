use crate::activity::CancellationToken;
use lumilio_plugin_api::map::TileKey;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug)]
pub struct Viewport {
    pub x: f64,
    pub z: f64,
    pub blocks_per_pixel: f64,
    pub width: u32,
    pub height: u32,
}

impl Viewport {
    pub fn visible(&self, template: &TileKey) -> Vec<TileKey> {
        if !self.x.is_finite()
            || !self.z.is_finite()
            || !self.blocks_per_pixel.is_finite()
            || self.blocks_per_pixel <= 0.0
            || self.blocks_per_pixel > 4096.0
            || self.x.abs() > 30_000_000.0
            || self.z.abs() > 30_000_000.0
            || self.width > 8192
            || self.height > 8192
        {
            return vec![];
        }
        let Some(scale) = template.blocks_per_pixel() else {
            return vec![];
        };
        let span = f64::from(scale) * 256.0;
        let rx = f64::from(self.width) * self.blocks_per_pixel / 2.0;
        let rz = f64::from(self.height) * self.blocks_per_pixel / 2.0;
        let min_x = ((self.x - rx) / span).floor() as i32 - 1;
        let max_x = ((self.x + rx) / span).floor() as i32 + 1;
        let min_z = ((self.z - rz) / span).floor() as i32 - 1;
        let max_z = ((self.z + rz) / span).floor() as i32 + 1;
        if i64::from(max_x) - i64::from(min_x) > 64 || i64::from(max_z) - i64::from(min_z) > 64 {
            return vec![];
        }
        let mut keys = Vec::new();
        for tx in min_x..=max_x {
            for tz in min_z..=max_z {
                keys.push(TileKey {
                    tx,
                    tz,
                    ..template.clone()
                });
            }
        }
        keys.sort_by(|a, b| {
            let distance = |key: &TileKey| {
                (f64::from(key.tx) + 0.5 - self.x / span).powi(2)
                    + (f64::from(key.tz) + 0.5 - self.z / span).powi(2)
            };
            distance(a).total_cmp(&distance(b)).then_with(|| a.cmp(b))
        });
        keys
    }
}

#[derive(Default)]
pub struct MapSchedule {
    generation: u64,
    visible: BTreeSet<TileKey>,
    pending: BTreeMap<TileKey, (u64, CancellationToken)>,
}
impl MapSchedule {
    pub fn update(&mut self, keys: &[TileKey]) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.visible = keys.iter().cloned().collect();
        self.pending.retain(|key, (_, token)| {
            let retained = self.visible.contains(key);
            if !retained {
                token.cancel();
            }
            retained
        });
        self.generation
    }
    /// Requests dispatched for the current viewport and not yet answered.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
    pub fn begin(&mut self, key: &TileKey) -> Option<(u64, CancellationToken)> {
        if !self.visible.contains(key) || self.pending.contains_key(key) {
            return None;
        }
        let token = CancellationToken::new();
        self.pending
            .insert(key.clone(), (self.generation, token.clone()));
        Some((self.generation, token))
    }
    pub fn accept(&mut self, generation: u64, key: &TileKey) -> bool {
        if !self.visible.contains(key)
            || self
                .pending
                .get(key)
                .is_none_or(|(pending_generation, _)| *pending_generation != generation)
        {
            return false;
        }
        // Cancellation also stops a timed-out/failing worker. Only removal
        // from this viewport invalidates its reply; a current error must reach
        // the user so the tile can explain the failure and offer a retry.
        self.pending.remove(key);
        true
    }
}
impl Drop for MapSchedule {
    fn drop(&mut self) {
        for (_, token) in self.pending.values() {
            token.cancel();
        }
    }
}
