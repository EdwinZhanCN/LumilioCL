//! Overlay objects for the viewport, fetched in fixed block cells so a pan
//! reuses what it already has. Cells are keyed by layer and world, requested
//! nearest first with a small in-flight limit, and dropped when they leave view.
use lumilio_core::CancellationToken;
use lumilio_plugin_api::map::{
    Dimension, MapBounds, MapObject, MapObjectKind, MapPoint, OverlayInfo, OverlayRequest,
    WorldContext, WorldId,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Cell edge in blocks. Structure search cost grows with area, and a cell this
/// size holds a few dozen candidates per kind.
const CELL: f64 = 4096.;
/// Icons are meaningless and cells too many beyond this many blocks per pixel.
pub(super) const MAX_SCALE: f64 = 16.;
const IN_FLIGHT: usize = 4;
const KEPT: usize = 512;

/// Names one cell of one layer in one world. Opaque outside this module; the
/// application only carries it between a request and its answer.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ObjectKey {
    overlay: String,
    world: WorldId,
    dimension: Dimension,
    x: i32,
    z: i32,
}

#[derive(Default)]
pub(super) struct Objects {
    /// The layers the current world offers, with the plugin that owns each.
    pub layers: Vec<(String, OverlayInfo)>,
    pub enabled: BTreeSet<String>,
    cells: BTreeMap<ObjectKey, Vec<MapObject>>,
    order: VecDeque<ObjectKey>,
    /// Cells whose request failed, with the failure's message id if it has one.
    failed: BTreeMap<ObjectKey, String>,
    pending: BTreeMap<ObjectKey, (u64, CancellationToken)>,
    wanted: Vec<ObjectKey>,
    generation: u64,
}

pub(super) struct Dispatch {
    pub generation: u64,
    pub plugin: String,
    pub key: ObjectKey,
    pub request: OverlayRequest,
    pub cancel: CancellationToken,
}

/// Whether a layer is drawn at this zoom, in blocks per pixel.
pub(super) fn shown_at(layer: &OverlayInfo, scale: f64) -> bool {
    scale <= layer.max_scale.map_or(MAX_SCALE, f64::from)
}

impl Objects {
    /// A link is missing only after the source's visible, enabled cells have
    /// answered. A hidden or still-loading layer cannot prove absence.
    pub fn link_missing(&self, source: &str, raw_id: &str) -> bool {
        let offered: Vec<&OverlayInfo> = self
            .layers
            .iter()
            .filter(|(plugin, _)| plugin == source)
            .map(|(_, layer)| layer)
            .collect();
        if offered.is_empty() {
            return true;
        }
        let active: BTreeSet<&str> = offered
            .iter()
            .filter(|layer| self.enabled.contains(&layer.id))
            .map(|layer| layer.id.as_str())
            .collect();
        if active.is_empty() {
            return false;
        }
        let keys: Vec<_> = self
            .wanted
            .iter()
            .filter(|key| active.contains(key.overlay.as_str()))
            .collect();
        if keys.is_empty()
            || keys.iter().any(|key| {
                self.pending.contains_key(*key)
                    || self.failed.contains_key(*key)
                    || !self.cells.contains_key(*key)
            })
        {
            return false;
        }
        !keys
            .iter()
            .filter_map(|key| self.cells.get(*key))
            .flatten()
            .any(|object| object.raw_id == raw_id)
    }

    pub fn new(enabled: &[&str]) -> Self {
        Self {
            enabled: enabled.iter().map(|id| (*id).to_owned()).collect(),
            ..Self::default()
        }
    }

    /// Forgets every cell and cancels what is in flight: the world changed.
    pub fn clear(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        for (_, token) in self.pending.values() {
            token.cancel();
        }
        self.pending.clear();
        self.cells.clear();
        self.order.clear();
        self.failed.clear();
        self.wanted.clear();
    }

    pub fn active(&self) -> impl Iterator<Item = &(String, OverlayInfo)> {
        self.layers
            .iter()
            .filter(|(_, layer)| self.enabled.contains(&layer.id))
    }

    pub fn toggle(&mut self, id: &str) {
        if !self.enabled.remove(id) {
            self.enabled.insert(id.to_owned());
        }
    }

    /// Recomputes which cells the view needs and cancels in-flight ones it no
    /// longer does. `view` is the viewport in blocks, `[x0, z0, x1, z1]`.
    pub fn update(&mut self, context: &WorldContext, view: [f64; 4], scale: f64) {
        self.generation = self.generation.wrapping_add(1);
        let mut wanted = Vec::new();
        if view.iter().all(|edge| edge.is_finite()) {
            let range = |lo: f64, hi: f64| (lo / CELL).floor() as i32..=(hi / CELL).floor() as i32;
            let (xs, zs) = (range(view[0], view[2]), range(view[1], view[3]));
            let centre = [(view[0] + view[2]) / 2., (view[1] + view[3]) / 2.];
            for (_, layer) in self.active().filter(|(_, layer)| shown_at(layer, scale)) {
                for x in xs.clone() {
                    for z in zs.clone() {
                        wanted.push(ObjectKey {
                            overlay: layer.id.clone(),
                            world: context.world.clone(),
                            dimension: context.dimension.clone(),
                            x,
                            z,
                        });
                    }
                }
            }
            let away = |key: &ObjectKey| {
                (f64::from(key.x) * CELL + CELL / 2. - centre[0]).powi(2)
                    + (f64::from(key.z) * CELL + CELL / 2. - centre[1]).powi(2)
            };
            wanted.sort_by(|a, b| away(a).total_cmp(&away(b)).then_with(|| a.cmp(b)));
        }
        let keep: BTreeSet<&ObjectKey> = wanted.iter().collect();
        self.pending.retain(|key, (_, token)| {
            let kept = keep.contains(key);
            if !kept {
                token.cancel();
            }
            kept
        });
        self.wanted = wanted;
    }

    /// Requests to start now: missing cells nearest the centre first, up to the
    /// in-flight limit.
    pub fn next(&mut self, context: &WorldContext) -> Vec<Dispatch> {
        let mut started = Vec::new();
        let wanted = self.wanted.clone();
        for key in wanted {
            if self.pending.len() >= IN_FLIGHT {
                break;
            }
            if self.cells.contains_key(&key)
                || self.failed.contains_key(&key)
                || self.pending.contains_key(&key)
            {
                continue;
            }
            let Some((plugin, _)) = self
                .layers
                .iter()
                .find(|(_, layer)| layer.id == key.overlay)
            else {
                continue;
            };
            let cancel = CancellationToken::new();
            self.pending
                .insert(key.clone(), (self.generation, cancel.clone()));
            let (x0, z0) = (f64::from(key.x) * CELL, f64::from(key.z) * CELL);
            started.push(Dispatch {
                generation: self.generation,
                plugin: plugin.clone(),
                request: OverlayRequest {
                    context: context.clone(),
                    overlay: key.overlay.clone(),
                    bounds: MapBounds {
                        min: MapPoint { x: x0, z: z0 },
                        max: MapPoint {
                            x: x0 + CELL,
                            z: z0 + CELL,
                        },
                    },
                    level: 0,
                },
                key,
                cancel,
            });
        }
        started
    }

    /// Whether an answer is still wanted: not cancelled, not from before a
    /// clear, and the cell is still in view.
    pub fn accept(&mut self, generation: u64, key: &ObjectKey) -> bool {
        match self.pending.get(key) {
            Some((pending, _)) if *pending == generation => {
                self.pending.remove(key);
                self.wanted.contains(key)
            }
            _ => false,
        }
    }

    pub fn store(&mut self, key: ObjectKey, objects: Vec<MapObject>) {
        self.failed.remove(&key);
        if self.cells.insert(key.clone(), objects).is_none() {
            self.order.push_back(key);
        }
        while self.order.len() > KEPT {
            let Some(old) = self.order.iter().position(|key| !self.wanted.contains(key)) else {
                break;
            };
            if let Some(old) = self.order.remove(old) {
                self.cells.remove(&old);
            }
        }
    }

    pub fn fail(&mut self, key: ObjectKey, reason: String) {
        self.failed.insert(key, reason);
    }

    /// Whether some failed cell failed with this message id.
    pub fn failed_with(&self, reason: &str) -> bool {
        self.failed.values().any(|known| known == reason)
    }

    pub fn failed(&self) -> usize {
        self.failed.len()
    }

    pub fn retry(&mut self) {
        self.failed.clear();
    }

    /// Objects of the wanted cells that fall inside `view`, lowest priority
    /// first so the important ones draw on top.
    pub fn visible(&self, view: [f64; 4]) -> Vec<&MapObject> {
        let mut found: Vec<&MapObject> = self
            .wanted
            .iter()
            .filter_map(|key| self.cells.get(key))
            .flatten()
            .filter(|object| match &object.kind {
                MapObjectKind::Icon { at, .. } => {
                    at.x >= view[0] && at.x <= view[2] && at.z >= view[1] && at.z <= view[3]
                }
                MapObjectKind::Heat { .. } => true,
                MapObjectKind::Polyline(points) => points.windows(2).any(|segment| {
                    let (a, b) = (segment[0], segment[1]);
                    a.x.min(b.x) <= view[2]
                        && a.x.max(b.x) >= view[0]
                        && a.z.min(b.z) <= view[3]
                        && a.z.max(b.z) >= view[1]
                }),
                _ => false,
            })
            .collect();
        let mut seen = BTreeSet::new();
        found.retain(|object| seen.insert((object.source.as_str(), object.id.as_str())));
        found.sort_by_key(|object| object.priority);
        found
    }
}
