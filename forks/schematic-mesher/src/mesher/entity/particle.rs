//! Static particle marker quads for torches, campfires, candles, etc.
//!
//! Renders small cross-quads (two intersecting diagonal quads) at known
//! particle spawn positions. Particle textures are composited into animated
//! sprite sheets so the existing animated texture viewer system can cycle
//! frames automatically.

use crate::mesher::geometry::Vertex;
use crate::resource_pack::{ResourcePack, TextureData};
use crate::resource_pack::texture::AnimationMeta;
use crate::types::InputBlock;
use super::EntityFaceTexture;

/// A single particle quad to render.
pub struct ParticleQuad {
    /// Center position in block-local [0,1] space.
    pub center: [f32; 3],
    /// Half-size of the quad.
    pub half_size: f32,
    /// Texture path — either raw (e.g. "particle/flame") or animated key
    /// (e.g. "_particle/flame"). The animated key is set by `add_particles()`.
    pub texture: &'static str,
}

/// Particle source detected from a block.
pub struct ParticleSource {
    pub quads: Vec<ParticleQuad>,
}

/// Animation definition for a particle texture type.
pub struct ParticleAnimDef {
    /// Synthetic key for the composited sprite sheet (e.g., "_particle/flame").
    pub key: &'static str,
    /// Individual frame texture paths in the resource pack.
    pub frames: &'static [&'static str],
    /// Ticks per frame (1 tick = 50ms).
    pub frametime: u32,
    /// If true, generate synthetic flicker frames from a single source texture
    /// instead of compositing numbered textures.
    pub synthetic_flicker: bool,
    /// Source texture for synthetic flicker (only used if synthetic_flicker is true).
    pub flicker_source: &'static str,
}

/// Get the animation definition for a particle texture, if one exists.
pub fn particle_anim_def(texture: &str) -> Option<&'static ParticleAnimDef> {
    match texture {
        "particle/flame" => Some(&FLAME_ANIM),
        "particle/soul_fire_flame" => Some(&SOUL_FLAME_ANIM),
        "particle/big_smoke_0" => Some(&BIG_SMOKE_ANIM),
        "particle/generic_0" => Some(&SMOKE_ANIM),
        "particle/glitter_0" => Some(&GLITTER_ANIM),
        _ => None,
    }
}

static FLAME_ANIM: ParticleAnimDef = ParticleAnimDef {
    key: "_particle/flame",
    frames: &[],
    frametime: 2,
    synthetic_flicker: true,
    flicker_source: "particle/flame",
};

static SOUL_FLAME_ANIM: ParticleAnimDef = ParticleAnimDef {
    key: "_particle/soul_fire_flame",
    frames: &[],
    frametime: 2,
    synthetic_flicker: true,
    flicker_source: "particle/soul_fire_flame",
};

static BIG_SMOKE_ANIM: ParticleAnimDef = ParticleAnimDef {
    key: "_particle/big_smoke",
    frames: &[
        "particle/big_smoke_0", "particle/big_smoke_1", "particle/big_smoke_2",
        "particle/big_smoke_3", "particle/big_smoke_4", "particle/big_smoke_5",
        "particle/big_smoke_6", "particle/big_smoke_7", "particle/big_smoke_8",
        "particle/big_smoke_9", "particle/big_smoke_10", "particle/big_smoke_11",
    ],
    frametime: 3,
    synthetic_flicker: false,
    flicker_source: "",
};

static SMOKE_ANIM: ParticleAnimDef = ParticleAnimDef {
    key: "_particle/smoke",
    frames: &[
        "particle/generic_0", "particle/generic_1", "particle/generic_2",
        "particle/generic_3", "particle/generic_4", "particle/generic_5",
        "particle/generic_6", "particle/generic_7",
    ],
    frametime: 3,
    synthetic_flicker: false,
    flicker_source: "",
};

static GLITTER_ANIM: ParticleAnimDef = ParticleAnimDef {
    key: "_particle/glitter",
    frames: &[
        "particle/glitter_0", "particle/glitter_1", "particle/glitter_2",
        "particle/glitter_3", "particle/glitter_4", "particle/glitter_5",
        "particle/glitter_6", "particle/glitter_7",
    ],
    frametime: 2,
    synthetic_flicker: false,
    flicker_source: "",
};

/// Brightness multipliers for synthetic flame flicker (8 frames).
const FLICKER_BRIGHTNESS: [f32; 8] = [1.0, 0.88, 1.0, 0.92, 0.85, 1.0, 0.90, 0.95];

/// Build an animated sprite sheet TextureData for a particle animation.
///
/// For multi-frame particles (smoke, glitter): composites numbered PNGs vertically.
/// For single-frame particles (flame): creates synthetic flicker by modulating brightness.
pub fn build_particle_sprite_sheet(
    resource_pack: &ResourcePack,
    anim: &ParticleAnimDef,
) -> Option<TextureData> {
    if anim.synthetic_flicker {
        build_flicker_sprite_sheet(resource_pack, anim)
    } else {
        build_composite_sprite_sheet(resource_pack, anim)
    }
}

/// Build a sprite sheet by compositing numbered frame textures vertically.
fn build_composite_sprite_sheet(
    resource_pack: &ResourcePack,
    anim: &ParticleAnimDef,
) -> Option<TextureData> {
    // Load all frame textures
    let mut frame_datas: Vec<TextureData> = Vec::new();
    for path in anim.frames {
        let tex = resource_pack.get_texture(path)?;
        frame_datas.push(tex.first_frame());
    }

    if frame_datas.is_empty() {
        return None;
    }

    let width = frame_datas[0].width;
    let height = frame_datas[0].height;
    let frame_count = frame_datas.len() as u32;

    // Stack frames vertically into a sprite sheet
    let total_height = height * frame_count;
    let mut pixels = Vec::with_capacity((width * total_height * 4) as usize);
    for frame in &frame_datas {
        if frame.width != width || frame.height != height {
            // Skip mismatched frames — use first frame dimensions
            pixels.extend_from_slice(&frame_datas[0].pixels);
        } else {
            pixels.extend_from_slice(&frame.pixels);
        }
    }

    let mut tex = TextureData::new(width, total_height, pixels);
    tex.is_animated = true;
    tex.frame_count = frame_count;
    tex.animation = Some(AnimationMeta {
        frametime: anim.frametime,
        interpolate: false,
        frames: None,
        frame_width: Some(width),
        frame_height: Some(height),
    });

    Some(tex)
}

/// Build a sprite sheet with synthetic brightness flicker for single-frame particles.
fn build_flicker_sprite_sheet(
    resource_pack: &ResourcePack,
    anim: &ParticleAnimDef,
) -> Option<TextureData> {
    let source = resource_pack.get_texture(anim.flicker_source)?;
    let base = source.first_frame();
    let width = base.width;
    let height = base.height;
    let frame_count = FLICKER_BRIGHTNESS.len() as u32;

    let total_height = height * frame_count;
    let mut pixels = Vec::with_capacity((width * total_height * 4) as usize);

    for &brightness in &FLICKER_BRIGHTNESS {
        for chunk in base.pixels.chunks(4) {
            let r = (chunk[0] as f32 * brightness).round().min(255.0) as u8;
            let g = (chunk[1] as f32 * brightness).round().min(255.0) as u8;
            let b = (chunk[2] as f32 * brightness).round().min(255.0) as u8;
            let a = chunk[3];
            pixels.extend_from_slice(&[r, g, b, a]);
        }
    }

    let mut tex = TextureData::new(width, total_height, pixels);
    tex.is_animated = true;
    tex.frame_count = frame_count;
    tex.animation = Some(AnimationMeta {
        frametime: anim.frametime,
        interpolate: false,
        frames: None,
        frame_width: Some(width),
        frame_height: Some(height),
    });

    Some(tex)
}

/// Detect if a block should produce static particle quads.
pub fn detect_particle_source(block: &InputBlock) -> Option<ParticleSource> {
    let block_id = block.block_id();

    match block_id {
        // Regular torch
        "torch" => Some(ParticleSource {
            quads: vec![ParticleQuad {
                center: [0.5, 0.7, 0.5],
                half_size: 0.12,
                texture: "particle/flame",
            }],
        }),

        // Soul torch
        "soul_torch" => Some(ParticleSource {
            quads: vec![ParticleQuad {
                center: [0.5, 0.7, 0.5],
                half_size: 0.12,
                texture: "particle/soul_fire_flame",
            }],
        }),

        // Wall torch — offset 0.27 from wall based on facing
        "wall_torch" => {
            let center = wall_torch_center(block, 0.7);
            Some(ParticleSource {
                quads: vec![ParticleQuad {
                    center,
                    half_size: 0.12,
                    texture: "particle/flame",
                }],
            })
        }

        // Soul wall torch
        "soul_wall_torch" => {
            let center = wall_torch_center(block, 0.7);
            Some(ParticleSource {
                quads: vec![ParticleQuad {
                    center,
                    half_size: 0.12,
                    texture: "particle/soul_fire_flame",
                }],
            })
        }

        // Campfire (only when lit)
        "campfire" | "soul_campfire" => {
            if block.properties.get("lit").map(|v| v == "true").unwrap_or(false) {
                Some(ParticleSource {
                    quads: vec![
                        ParticleQuad {
                            center: [0.5, 0.8, 0.5],
                            half_size: 0.15,
                            texture: "particle/big_smoke_0",
                        },
                        ParticleQuad {
                            center: [0.5, 1.1, 0.5],
                            half_size: 0.12,
                            texture: "particle/big_smoke_0",
                        },
                        ParticleQuad {
                            center: [0.5, 1.4, 0.5],
                            half_size: 0.10,
                            texture: "particle/big_smoke_0",
                        },
                    ],
                })
            } else {
                None
            }
        }

        // Candles (lit only, 1-4 candles with per-wick positions)
        id if id == "candle" || id.ends_with("_candle") => {
            if block.properties.get("lit").map(|v| v == "true").unwrap_or(false) {
                let count: u8 = block.properties.get("candles")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1);
                let quads = candle_positions(count)
                    .into_iter()
                    .map(|center| ParticleQuad {
                        center,
                        half_size: 0.06,
                        texture: "particle/flame",
                    })
                    .collect();
                Some(ParticleSource { quads })
            } else {
                None
            }
        }

        // Lantern
        "lantern" => Some(ParticleSource {
            quads: vec![ParticleQuad {
                center: [0.5, 0.4, 0.5],
                half_size: 0.08,
                texture: "particle/flame",
            }],
        }),

        // Soul lantern
        "soul_lantern" => Some(ParticleSource {
            quads: vec![ParticleQuad {
                center: [0.5, 0.4, 0.5],
                half_size: 0.08,
                texture: "particle/soul_fire_flame",
            }],
        }),

        // End rod — tip based on facing, uses glitter textures
        "end_rod" => {
            let center = end_rod_tip(block);
            Some(ParticleSource {
                quads: vec![ParticleQuad {
                    center,
                    half_size: 0.06,
                    texture: "particle/glitter_0",
                }],
            })
        }

        // Furnace variants (lit only)
        "furnace" | "smoker" | "blast_furnace" => {
            if block.properties.get("lit").map(|v| v == "true").unwrap_or(false) {
                let facing = block.properties.get("facing")
                    .map(|s| s.as_str())
                    .unwrap_or("north");
                let mut quads = Vec::new();
                // Flame at front face center
                let front = furnace_front_center(facing);
                quads.push(ParticleQuad {
                    center: front,
                    half_size: 0.08,
                    texture: "particle/flame",
                });
                // Smoke rising from top (uses generic_0..7 sequence)
                quads.push(ParticleQuad {
                    center: [0.5, 1.05, 0.5],
                    half_size: 0.10,
                    texture: "particle/generic_0",
                });
                Some(ParticleSource { quads })
            } else {
                None
            }
        }

        _ => None,
    }
}

/// Wall torch flame position based on facing direction.
/// `facing` indicates the direction the torch points (not the wall it's on).
fn wall_torch_center(block: &InputBlock, y: f32) -> [f32; 3] {
    let facing = block.properties.get("facing")
        .map(|s| s.as_str())
        .unwrap_or("north");
    // Flame is near the wall (torch only tilts 22.5°, tip stays close).
    // MC uses: 0.5 + 0.27 * opposite_direction
    match facing {
        "north" => [0.5, y, 0.73],  // wall on south side (+Z), flame near wall
        "south" => [0.5, y, 0.27],  // wall on north side (-Z), flame near wall
        "east" => [0.27, y, 0.5],   // wall on west side (-X), flame near wall
        "west" => [0.73, y, 0.5],   // wall on east side (+X), flame near wall
        _ => [0.5, y, 0.5],
    }
}

/// Candle wick positions based on count (1-4).
/// Positions from Minecraft's CandleBlock.java.
fn candle_positions(count: u8) -> Vec<[f32; 3]> {
    match count {
        1 => vec![[0.5, 0.5, 0.5]],
        2 => vec![
            [0.375, 0.44, 0.5],
            [0.625, 0.5, 0.5],
        ],
        3 => vec![
            [0.5, 0.5, 0.375],
            [0.375, 0.44, 0.625],
            [0.6875, 0.44, 0.5625],
        ],
        4 => vec![
            [0.375, 0.44, 0.375],
            [0.625, 0.5, 0.375],
            [0.375, 0.5, 0.625],
            [0.625, 0.44, 0.625],
        ],
        _ => vec![[0.5, 0.5, 0.5]],
    }
}

/// End rod tip position based on facing.
fn end_rod_tip(block: &InputBlock) -> [f32; 3] {
    let facing = block.properties.get("facing")
        .map(|s| s.as_str())
        .unwrap_or("up");
    match facing {
        "up" => [0.5, 0.95, 0.5],
        "down" => [0.5, 0.05, 0.5],
        "north" => [0.5, 0.5, 0.05],
        "south" => [0.5, 0.5, 0.95],
        "east" => [0.95, 0.5, 0.5],
        "west" => [0.05, 0.5, 0.5],
        _ => [0.5, 0.95, 0.5],
    }
}

/// Furnace front face center based on facing.
fn furnace_front_center(facing: &str) -> [f32; 3] {
    match facing {
        "north" => [0.5, 0.4, 0.02],
        "south" => [0.5, 0.4, 0.98],
        "east" => [0.98, 0.4, 0.5],
        "west" => [0.02, 0.4, 0.5],
        _ => [0.5, 0.4, 0.02],
    }
}

/// Generate cross-quad geometry for all particle quads in a source.
///
/// Each cross-quad is 2 diagonal quads × 2 sides (double-sided) = 8 triangles.
/// Returns (vertices, indices, face_textures).
pub fn generate_particle_geometry(
    source: &ParticleSource,
) -> (Vec<Vertex>, Vec<u32>, Vec<EntityFaceTexture>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut face_textures = Vec::new();

    for quad in &source.quads {
        generate_cross_quad(
            quad.center,
            quad.half_size,
            quad.texture,
            &mut vertices,
            &mut indices,
            &mut face_textures,
        );
    }

    (vertices, indices, face_textures)
}

/// Generate a cross-quad: two intersecting diagonal quads, each double-sided.
///
/// Produces 4 quads (16 vertices, 24 indices, 4 face textures).
fn generate_cross_quad(
    center: [f32; 3],
    half: f32,
    texture: &str,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    face_textures: &mut Vec<EntityFaceTexture>,
) {
    let [cx, cy, cz] = center;

    // Diagonal 1: (cx-s, cy, cz-s) to (cx+s, cy, cz+s) — NW-SE diagonal
    emit_double_sided_quad(
        [cx - half, cy - half, cz - half],
        [cx + half, cy - half, cz + half],
        [cx + half, cy + half, cz + half],
        [cx - half, cy + half, cz - half],
        texture,
        vertices,
        indices,
        face_textures,
    );

    // Diagonal 2: (cx+s, cy, cz-s) to (cx-s, cy, cz+s) — NE-SW diagonal
    emit_double_sided_quad(
        [cx + half, cy - half, cz - half],
        [cx - half, cy - half, cz + half],
        [cx - half, cy + half, cz + half],
        [cx + half, cy + half, cz - half],
        texture,
        vertices,
        indices,
        face_textures,
    );
}

/// Emit a double-sided quad (front + back face).
///
/// Produces 2 quads (8 vertices, 12 indices, 2 face textures).
fn emit_double_sided_quad(
    bl: [f32; 3],
    br: [f32; 3],
    tr: [f32; 3],
    tl: [f32; 3],
    texture: &str,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    face_textures: &mut Vec<EntityFaceTexture>,
) {
    // Compute normal from cross product
    let e1 = [br[0] - bl[0], br[1] - bl[1], br[2] - bl[2]];
    let e2 = [tl[0] - bl[0], tl[1] - bl[1], tl[2] - bl[2]];
    let nx = e1[1] * e2[2] - e1[2] * e2[1];
    let ny = e1[2] * e2[0] - e1[0] * e2[2];
    let nz = e1[0] * e2[1] - e1[1] * e2[0];
    let len = (nx * nx + ny * ny + nz * nz).sqrt().max(0.001);
    let normal = [nx / len, ny / len, nz / len];
    let back_normal = [-normal[0], -normal[1], -normal[2]];

    let white = [1.0f32, 1.0, 1.0, 1.0];

    // UVs: simple [0,1] quad
    let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

    // Front face
    let v_start = vertices.len() as u32;
    vertices.push(Vertex::new(bl, normal, uvs[0]).with_color(white));
    vertices.push(Vertex::new(br, normal, uvs[1]).with_color(white));
    vertices.push(Vertex::new(tr, normal, uvs[2]).with_color(white));
    vertices.push(Vertex::new(tl, normal, uvs[3]).with_color(white));
    indices.extend_from_slice(&[
        v_start, v_start + 1, v_start + 2,
        v_start, v_start + 2, v_start + 3,
    ]);
    face_textures.push(EntityFaceTexture {
        texture: texture.to_string(),
        is_transparent: true,
    });

    // Back face (reversed winding, flipped normal)
    let v_start = vertices.len() as u32;
    vertices.push(Vertex::new(bl, back_normal, uvs[0]).with_color(white));
    vertices.push(Vertex::new(br, back_normal, uvs[1]).with_color(white));
    vertices.push(Vertex::new(tr, back_normal, uvs[2]).with_color(white));
    vertices.push(Vertex::new(tl, back_normal, uvs[3]).with_color(white));
    indices.extend_from_slice(&[
        v_start, v_start + 2, v_start + 1,
        v_start, v_start + 3, v_start + 2,
    ]);
    face_textures.push(EntityFaceTexture {
        texture: texture.to_string(),
        is_transparent: true,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_torch() {
        let block = InputBlock::new("minecraft:torch");
        let source = detect_particle_source(&block).unwrap();
        assert_eq!(source.quads.len(), 1);
        assert_eq!(source.quads[0].texture, "particle/flame");
    }

    #[test]
    fn test_detect_soul_torch() {
        let block = InputBlock::new("minecraft:soul_torch");
        let source = detect_particle_source(&block).unwrap();
        assert_eq!(source.quads[0].texture, "particle/soul_fire_flame");
    }

    #[test]
    fn test_detect_wall_torch() {
        let block = InputBlock::new("minecraft:wall_torch")
            .with_property("facing", "north");
        let source = detect_particle_source(&block).unwrap();
        assert_eq!(source.quads.len(), 1);
        // North facing: flame near south wall (+Z side), so z > 0.5
        assert!(source.quads[0].center[2] > 0.5);
    }

    #[test]
    fn test_detect_campfire_lit() {
        let block = InputBlock::new("minecraft:campfire")
            .with_property("lit", "true");
        let source = detect_particle_source(&block).unwrap();
        assert_eq!(source.quads.len(), 3); // 3 rising smoke quads
    }

    #[test]
    fn test_detect_campfire_unlit() {
        let block = InputBlock::new("minecraft:campfire")
            .with_property("lit", "false");
        assert!(detect_particle_source(&block).is_none());
    }

    #[test]
    fn test_detect_candle_lit() {
        let block = InputBlock::new("minecraft:candle")
            .with_property("lit", "true")
            .with_property("candles", "3");
        let source = detect_particle_source(&block).unwrap();
        assert_eq!(source.quads.len(), 3);
    }

    #[test]
    fn test_detect_colored_candle() {
        let block = InputBlock::new("minecraft:red_candle")
            .with_property("lit", "true")
            .with_property("candles", "2");
        let source = detect_particle_source(&block).unwrap();
        assert_eq!(source.quads.len(), 2);
    }

    #[test]
    fn test_detect_candle_unlit() {
        let block = InputBlock::new("minecraft:candle")
            .with_property("lit", "false");
        assert!(detect_particle_source(&block).is_none());
    }

    #[test]
    fn test_detect_lantern() {
        let block = InputBlock::new("minecraft:lantern");
        let source = detect_particle_source(&block).unwrap();
        assert_eq!(source.quads.len(), 1);
        assert_eq!(source.quads[0].texture, "particle/flame");
    }

    #[test]
    fn test_detect_soul_lantern() {
        let block = InputBlock::new("minecraft:soul_lantern");
        let source = detect_particle_source(&block).unwrap();
        assert_eq!(source.quads[0].texture, "particle/soul_fire_flame");
    }

    #[test]
    fn test_detect_end_rod() {
        let block = InputBlock::new("minecraft:end_rod")
            .with_property("facing", "up");
        let source = detect_particle_source(&block).unwrap();
        assert_eq!(source.quads[0].texture, "particle/glitter_0");
        assert!(source.quads[0].center[1] > 0.9); // tip near top
    }

    #[test]
    fn test_detect_furnace_lit() {
        let block = InputBlock::new("minecraft:furnace")
            .with_property("lit", "true")
            .with_property("facing", "north");
        let source = detect_particle_source(&block).unwrap();
        assert_eq!(source.quads.len(), 2); // flame + smoke
        assert_eq!(source.quads[1].texture, "particle/generic_0");
    }

    #[test]
    fn test_detect_furnace_unlit() {
        let block = InputBlock::new("minecraft:furnace")
            .with_property("lit", "false");
        assert!(detect_particle_source(&block).is_none());
    }

    #[test]
    fn test_detect_smoker() {
        let block = InputBlock::new("minecraft:smoker")
            .with_property("lit", "true")
            .with_property("facing", "south");
        let source = detect_particle_source(&block).unwrap();
        assert_eq!(source.quads.len(), 2);
    }

    #[test]
    fn test_detect_stone_no_particles() {
        let block = InputBlock::new("minecraft:stone");
        assert!(detect_particle_source(&block).is_none());
    }

    #[test]
    fn test_cross_quad_geometry_counts() {
        // Single particle quad → 1 cross-quad → 2 diagonals × 2 sides = 4 quads
        let source = ParticleSource {
            quads: vec![ParticleQuad {
                center: [0.5, 0.5, 0.5],
                half_size: 0.1,
                texture: "particle/flame",
            }],
        };
        let (verts, indices, faces) = generate_particle_geometry(&source);
        // 4 quads × 4 vertices = 16
        assert_eq!(verts.len(), 16);
        // 4 quads × 6 indices = 24
        assert_eq!(indices.len(), 24);
        // 4 face textures
        assert_eq!(faces.len(), 4);
        // All faces should be transparent
        assert!(faces.iter().all(|f| f.is_transparent));
    }

    #[test]
    fn test_multiple_quads_geometry() {
        // Campfire has 3 smoke quads → 3 × 4 = 12 quads total
        let source = ParticleSource {
            quads: vec![
                ParticleQuad { center: [0.5, 0.8, 0.5], half_size: 0.15, texture: "particle/big_smoke_0" },
                ParticleQuad { center: [0.5, 1.1, 0.5], half_size: 0.12, texture: "particle/big_smoke_0" },
                ParticleQuad { center: [0.5, 1.4, 0.5], half_size: 0.10, texture: "particle/big_smoke_0" },
            ],
        };
        let (verts, indices, faces) = generate_particle_geometry(&source);
        assert_eq!(verts.len(), 48);  // 3 × 16
        assert_eq!(indices.len(), 72); // 3 × 24
        assert_eq!(faces.len(), 12);   // 3 × 4
    }

    #[test]
    fn test_vertex_colors_are_white() {
        let source = ParticleSource {
            quads: vec![ParticleQuad {
                center: [0.5, 0.5, 0.5],
                half_size: 0.1,
                texture: "particle/flame",
            }],
        };
        let (verts, _, _) = generate_particle_geometry(&source);
        for v in &verts {
            assert_eq!(v.color, [1.0, 1.0, 1.0, 1.0]);
        }
    }

    #[test]
    fn test_particle_anim_def_flame() {
        let anim = particle_anim_def("particle/flame").unwrap();
        assert_eq!(anim.key, "_particle/flame");
        assert!(anim.synthetic_flicker);
    }

    #[test]
    fn test_particle_anim_def_smoke() {
        let anim = particle_anim_def("particle/big_smoke_0").unwrap();
        assert_eq!(anim.key, "_particle/big_smoke");
        assert!(!anim.synthetic_flicker);
        assert_eq!(anim.frames.len(), 12);
    }

    #[test]
    fn test_particle_anim_def_glitter() {
        let anim = particle_anim_def("particle/glitter_0").unwrap();
        assert_eq!(anim.key, "_particle/glitter");
        assert_eq!(anim.frames.len(), 8);
    }

    #[test]
    fn test_particle_anim_def_generic_smoke() {
        let anim = particle_anim_def("particle/generic_0").unwrap();
        assert_eq!(anim.key, "_particle/smoke");
        assert_eq!(anim.frames.len(), 8);
    }

    #[test]
    fn test_particle_anim_def_none() {
        assert!(particle_anim_def("block/stone").is_none());
    }
}
