//! Scenario-replay animation: turn an initial block set + a tick-stamped event
//! timeline into an animated GLB.
//!
//! The timeline is the neutral, MC-version-agnostic event vocabulary decoded
//! from an MCAP capture (raw clientbound packets) on the mod side:
//!
//! - [`TimelineEvent::SetBlock`] — a block at `pos` becomes `block` at `tick`
//!   (a repeater powering on, a lamp lighting, a piston base extending …).
//! - [`TimelineEvent::Piston`] — a piston at `pos` extends/retracts in `dir` at
//!   `tick`. The blocks it pushes are reconstructed here by walking the push
//!   column against the running world state.
//!
//! Output strategy (see [`crate::export::gltf_animated`]):
//! - blocks that never change → one static base node;
//! - a position that changes state → one node per state it visits, toggled
//!   on/off with a STEP `scale` track;
//! - a block pushed by a piston → one node with a LINEAR `translation` track.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::export::gltf_animated::{export_animated_glb, AnimatedPiece};
use crate::mesher::{Mesher, MesherConfig};
use crate::resource_pack::ResourcePack;
use crate::types::{BlockPosition, BoundingBox, Direction, InputBlock};

/// Piston extension/retraction duration, in game ticks. Vanilla piston arm
/// motion is ~2 ticks; 3 gives a slightly smoother slide for the replay.
const PUSH_TICKS: u32 = 3;
/// How long after a piston event its resolution `SetBlock` packets keep
/// arriving (batched `section_blocks_update`). `SetBlock`s for piston-involved
/// positions inside this window are the push resolution — already animated by
/// the translation track — so they're dropped instead of double-animated.
const RESOLVE_GRACE: u32 = 6;
/// Max blocks a piston can push (vanilla limit).
const PUSH_LIMIT: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PistonAction {
    Extend,
    Retract,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TimelineEvent {
    SetBlock {
        tick: u32,
        /// World coordinates.
        pos: [i32; 3],
        block: String,
        #[serde(default)]
        props: HashMap<String, String>,
    },
    Piston {
        tick: u32,
        /// World coordinates of the piston base.
        pos: [i32; 3],
        action: PistonAction,
        dir: Direction,
    },
}

impl TimelineEvent {
    fn tick(&self) -> u32 {
        match self {
            TimelineEvent::SetBlock { tick, .. } => *tick,
            TimelineEvent::Piston { tick, .. } => *tick,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timeline {
    /// Capture region min corner (world coords). model-local = world − origin.
    pub origin: [i32; 3],
    /// Milliseconds per game tick — maps tick numbers to animation seconds.
    pub tick_ms: f32,
    pub events: Vec<TimelineEvent>,
}

fn add(a: [i32; 3], b: (i32, i32, i32)) -> [i32; 3] {
    [a[0] + b.0, a[1] + b.1, a[2] + b.2]
}

fn input_block(name: &str, props: &HashMap<String, String>) -> InputBlock {
    let mut b = InputBlock::new(name.to_string());
    for (k, v) in props {
        b.properties.insert(k.clone(), v.clone());
    }
    b
}

/// A piston head block facing `dir` (the direction the piston points).
fn piston_head(dir: Direction) -> InputBlock {
    InputBlock::new("minecraft:piston_head")
        .with_property("facing", format!("{:?}", dir).to_lowercase())
        .with_property("type", "normal")
        .with_property("short", "false")
}

/// A node we still need to mesh: a block at a model-local position, plus the
/// keyframe tracks that drive it.
struct PendingPiece {
    block: InputBlock,
    pos_local: [i32; 3],
    scale_keys: Option<Vec<(f32, [f32; 3])>>,
    translation_keys: Option<Vec<(f32, [f32; 3])>>,
}

/// Build an animated GLB from the schematic's initial blocks (model-local /
/// schematic-local coords) and a decoded event timeline.
pub fn build_animated_glb(
    pack: &ResourcePack,
    initial: &[(BlockPosition, InputBlock)],
    timeline: &Timeline,
) -> Result<Vec<u8>> {
    let origin = timeline.origin;
    let tick_t = |tick: u32| (tick as f32) * timeline.tick_ms / 1000.0;

    // running world state, model-local coords
    let mut world: HashMap<[i32; 3], InputBlock> = initial
        .iter()
        .filter(|(_, b)| !b.is_air())
        .map(|(p, b)| ([p.x, p.y, p.z], b.clone()))
        .collect();
    let initial_world = world.clone();

    // events sorted by tick (stable — preserves intra-tick packet order)
    let mut events: Vec<&TimelineEvent> = timeline.events.iter().collect();
    events.sort_by_key(|e| e.tick());
    let last_tick = events.last().map(|e| e.tick()).unwrap_or(0);
    let total_t = tick_t(last_tick + PUSH_TICKS + 1);

    // per-position state stops (model-local), in tick order
    let mut state_stops: HashMap<[i32; 3], Vec<(u32, InputBlock)>> = HashMap::new();
    // motion pieces (pushed blocks + piston heads)
    let mut motions: Vec<PendingPiece> = Vec::new();
    // positions whose SetBlock resolution is handled by a piston motion:
    // pos -> window-end tick
    let mut consumed: HashMap<[i32; 3], u32> = HashMap::new();

    for ev in &events {
        match ev {
            TimelineEvent::SetBlock {
                tick, pos, block, props,
            } => {
                let lp = [pos[0] - origin[0], pos[1] - origin[1], pos[2] - origin[2]];
                if let Some(&until) = consumed.get(&lp) {
                    if *tick <= until {
                        // piston push resolution — already animated by a motion
                        continue;
                    }
                }
                let blk = input_block(block, props);
                if blk.is_air() {
                    world.remove(&lp);
                } else {
                    world.insert(lp, blk.clone());
                }
                state_stops.entry(lp).or_default().push((*tick, blk));
            }
            TimelineEvent::Piston {
                tick, pos, action, dir,
            } => {
                let base = [pos[0] - origin[0], pos[1] - origin[1], pos[2] - origin[2]];
                let off = dir.offset();
                match action {
                    PistonAction::Extend => {
                        // walk the contiguous push column ahead of the piston
                        let mut column: Vec<[i32; 3]> = Vec::new();
                        let mut q = add(base, off);
                        while world.contains_key(&q) && column.len() < PUSH_LIMIT {
                            column.push(q);
                            q = add(q, off);
                        }
                        let t0 = tick_t(*tick);
                        let t1 = tick_t(tick + PUSH_TICKS);
                        let delta = [off.0 as f32, off.1 as f32, off.2 as f32];
                        // each pushed block slides +dir over [t0, t1]
                        for &cpos in &column {
                            let blk = world.get(&cpos).cloned().unwrap();
                            motions.push(PendingPiece {
                                block: blk,
                                pos_local: cpos,
                                scale_keys: None,
                                translation_keys: Some(vec![
                                    (0.0, [0.0; 3]),
                                    (t0, [0.0; 3]),
                                    (t1, delta),
                                    (total_t, delta),
                                ]),
                            });
                            consumed.insert(cpos, tick + PUSH_TICKS + RESOLVE_GRACE);
                            consumed.insert(add(cpos, off), tick + PUSH_TICKS + RESOLVE_GRACE);
                        }
                        // piston head: appears at `base` and slides to `base+dir`
                        let head_pos = add(base, off);
                        let neg = [-delta[0], -delta[1], -delta[2]];
                        motions.push(PendingPiece {
                            block: piston_head(*dir),
                            pos_local: head_pos,
                            scale_keys: Some(vec![(0.0, [0.0; 3]), (t0, [1.0; 3])]),
                            translation_keys: Some(vec![
                                (0.0, neg),
                                (t0, neg),
                                (t1, [0.0; 3]),
                                (total_t, [0.0; 3]),
                            ]),
                        });
                        consumed.insert(head_pos, tick + PUSH_TICKS + RESOLVE_GRACE);
                        // advance running world: shift the column +dir, head at base+dir
                        for &cpos in column.iter().rev() {
                            if let Some(b) = world.remove(&cpos) {
                                world.insert(add(cpos, off), b);
                            }
                        }
                        world.insert(head_pos, piston_head(*dir));
                    }
                    PistonAction::Retract => {
                        // head disappears; a sticky piston pulls the block in
                        // front back by one. v1: animate the head retracting and
                        // pull whatever sits at base+2·dir back to base+dir.
                        let head_pos = add(base, off);
                        let pulled_from = add(head_pos, off);
                        let t0 = tick_t(*tick);
                        let t1 = tick_t(tick + PUSH_TICKS);
                        let delta = [off.0 as f32, off.1 as f32, off.2 as f32];
                        let neg = [-delta[0], -delta[1], -delta[2]];
                        // head slides base+dir → base, then vanishes
                        motions.push(PendingPiece {
                            block: piston_head(*dir),
                            pos_local: base,
                            scale_keys: Some(vec![(0.0, [1.0; 3]), (t1, [0.0; 3])]),
                            translation_keys: Some(vec![
                                (0.0, delta),
                                (t0, delta),
                                (t1, [0.0; 3]),
                                (total_t, [0.0; 3]),
                            ]),
                        });
                        consumed.insert(head_pos, tick + PUSH_TICKS + RESOLVE_GRACE);
                        if let Some(blk) = world.get(&pulled_from).cloned() {
                            motions.push(PendingPiece {
                                block: blk.clone(),
                                pos_local: pulled_from,
                                scale_keys: None,
                                translation_keys: Some(vec![
                                    (0.0, [0.0; 3]),
                                    (t0, [0.0; 3]),
                                    (t1, neg),
                                    (total_t, neg),
                                ]),
                            });
                            consumed.insert(pulled_from, tick + PUSH_TICKS + RESOLVE_GRACE);
                            consumed.insert(head_pos, tick + PUSH_TICKS + RESOLVE_GRACE);
                            world.remove(&pulled_from);
                            world.insert(head_pos, blk);
                        } else {
                            world.remove(&head_pos);
                        }
                    }
                }
            }
        }
    }

    // ---- decide static vs dynamic positions ----
    let dynamic: HashSet<[i32; 3]> = state_stops
        .keys()
        .copied()
        .chain(motions.iter().map(|m| m.pos_local))
        .collect();

    // ---- collect every distinct block that ever appears (for one shared atlas) ----
    let mut palette: Vec<InputBlock> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut note = |b: &InputBlock, palette: &mut Vec<InputBlock>, seen: &mut HashSet<String>| {
        let key = palette_key(b);
        if seen.insert(key) {
            palette.push(b.clone());
        }
    };
    for (_, b) in &initial_world {
        note(b, &mut palette, &mut seen);
    }
    for stops in state_stops.values() {
        for (_, b) in stops {
            note(b, &mut palette, &mut seen);
        }
    }
    for m in &motions {
        note(&m.block, &mut palette, &mut seen);
    }

    // ---- build one shared atlas by meshing the whole palette once ----
    let atlas = {
        let cfg = MesherConfig {
            cull_hidden_faces: false,
            cull_occluded_blocks: false,
            greedy_meshing: false,
            ..Default::default()
        };
        let mesher = Mesher::with_config(pack.clone(), cfg);
        let spaced: Vec<(BlockPosition, InputBlock)> = palette
            .iter()
            .enumerate()
            .map(|(i, b)| (BlockPosition::new((i as i32) * 2, 0, 0), b.clone()))
            .collect();
        let bounds = BoundingBox::new(
            [0.0, 0.0, 0.0],
            [(palette.len().max(1) as f32) * 2.0, 1.0, 1.0],
        );
        let out = mesher.mesh_blocks(spaced.iter().map(|(p, b)| (*p, b)), bounds)?;
        out.atlas
    };

    // mesher that remaps every per-piece mesh onto the shared atlas
    let piece_cfg = MesherConfig {
        cull_hidden_faces: false,
        cull_occluded_blocks: false,
        greedy_meshing: false,
        pre_built_atlas: Some(atlas.clone()),
        ..Default::default()
    };
    let piece_mesher = Mesher::with_config(pack.clone(), piece_cfg);

    // helper: mesh one block at a model-local position → a merged Mesh
    let mesh_one = |block: &InputBlock, p: [i32; 3]| -> Result<crate::mesher::Mesh> {
        let bp = BlockPosition::new(p[0], p[1], p[2]);
        let bounds = BoundingBox::new(
            [p[0] as f32, p[1] as f32, p[2] as f32],
            [p[0] as f32 + 1.0, p[1] as f32 + 1.0, p[2] as f32 + 1.0],
        );
        let out = piece_mesher.mesh_blocks(std::iter::once((bp, block)), bounds)?;
        Ok(out.mesh())
    };

    let mut pieces: Vec<AnimatedPiece> = Vec::new();

    // 1. static base — every initial block at a non-dynamic position, meshed together
    {
        let static_blocks: Vec<(BlockPosition, InputBlock)> = initial_world
            .iter()
            .filter(|(p, _)| !dynamic.contains(*p))
            .map(|(p, b)| (BlockPosition::new(p[0], p[1], p[2]), b.clone()))
            .collect();
        if !static_blocks.is_empty() {
            let bounds = bounds_of(static_blocks.iter().map(|(p, _)| *p));
            let out = piece_mesher.mesh_blocks(static_blocks.iter().map(|(p, b)| (*p, b)), bounds)?;
            pieces.push(AnimatedPiece {
                mesh: out.mesh(),
                scale_keys: None,
                translation_keys: None,
            });
        }
    }

    // 2. state-change variants — one node per (position, state interval)
    for (&pos, stops) in &state_stops {
        // full state sequence: the initial block, then each stop
        let mut seq: Vec<(u32, Option<InputBlock>)> = Vec::new();
        if let Some(b) = initial_world.get(&pos) {
            seq.push((0, Some(b.clone())));
        } else {
            seq.push((0, None));
        }
        for (t, b) in stops {
            seq.push((*t, if b.is_air() { None } else { Some(b.clone()) }));
        }
        for i in 0..seq.len() {
            let (start_tick, ref block) = seq[i];
            let Some(block) = block else { continue };
            let end_tick = seq.get(i + 1).map(|(t, _)| *t);
            let start_t = tick_t(start_tick);
            let mesh = mesh_one(block, pos)?;
            if mesh.is_empty() {
                continue;
            }
            // STEP scale track: visible only on [start_t, end_t)
            let mut keys: Vec<(f32, [f32; 3])> = Vec::new();
            if start_t > 0.0 {
                keys.push((0.0, [0.0; 3]));
            }
            keys.push((start_t, [1.0; 3]));
            match end_tick {
                Some(et) => keys.push((tick_t(et), [0.0; 3])),
                None => keys.push((total_t, [1.0; 3])),
            }
            pieces.push(AnimatedPiece {
                mesh,
                scale_keys: Some(keys),
                translation_keys: None,
            });
        }
    }

    // 3. motion pieces — pushed blocks + piston heads
    for m in &motions {
        let mesh = mesh_one(&m.block, m.pos_local)?;
        if mesh.is_empty() {
            continue;
        }
        pieces.push(AnimatedPiece {
            mesh,
            scale_keys: m.scale_keys.clone(),
            translation_keys: m.translation_keys.clone(),
        });
    }

    export_animated_glb(&atlas, &pieces)
}

/// Stable key for atlas-palette dedup: name + sorted properties.
fn palette_key(b: &InputBlock) -> String {
    let mut props: Vec<(&String, &String)> = b.properties.iter().collect();
    props.sort();
    let mut s = b.name.clone();
    for (k, v) in props {
        s.push('|');
        s.push_str(k);
        s.push('=');
        s.push_str(v);
    }
    s
}

fn bounds_of(positions: impl Iterator<Item = BlockPosition>) -> BoundingBox {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    let mut any = false;
    for p in positions {
        any = true;
        let v = [p.x as f32, p.y as f32, p.z as f32];
        for i in 0..3 {
            min[i] = min[i].min(v[i]);
            max[i] = max[i].max(v[i] + 1.0);
        }
    }
    if !any {
        return BoundingBox::new([0.0; 3], [1.0; 3]);
    }
    BoundingBox::new(min, max)
}
