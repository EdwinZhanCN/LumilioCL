use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose, SkullType};
use crate::resource_pack::TextureData;
use crate::types::InputBlock;

/// Skull/head model.
pub(super) fn skull_model(skull_type: SkullType) -> EntityModelDef {
    let texture_path = match skull_type {
        SkullType::Skeleton => "entity/skeleton/skeleton",
        SkullType::WitherSkeleton => "entity/skeleton/wither_skeleton",
        SkullType::Zombie => "entity/zombie/zombie",
        SkullType::Creeper => "entity/creeper/creeper",
        SkullType::Piglin => "entity/piglin/piglin",
        SkullType::Dragon => "entity/enderdragon/dragon",
        SkullType::Player => "entity/player/wide/steve", // fallback, overridden by add_player_head
    };

    let texture_size: [u32; 2] = match skull_type {
        SkullType::Piglin => [64, 64],
        SkullType::Dragon => [256, 256],
        SkullType::Zombie | SkullType::Player => [64, 64],
        _ => [64, 32],
    };

    // Head: 8x8x8 at texOffs(0,0)
    // Skull uses Y-down entity coords — wrap in root with RotX(PI) to flip Y-up.
    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -8.0, -4.0],
            dimensions: [8.0, 8.0, 8.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let mut inner_parts = vec![head];

    // Hat overlay for types that have hat texture region (zombie, piglin, player).
    let has_hat = matches!(skull_type, SkullType::Zombie | SkullType::Piglin | SkullType::Player);
    if has_hat {
        inner_parts.push(EntityPart {
            cubes: vec![EntityCube {
                origin: [-4.0, -8.0, -4.0],
                dimensions: [8.0, 8.0, 8.0],
                tex_offset: [32, 0],
                inflate: 0.25,
                mirror: false,
                skip_faces: vec![],
            }],
            pose: Default::default(),
            children: vec![],
        });
    }

    // Root wrapper: RotX(PI) for Y-down→Y-up, position centers skull on block.
    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 0.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children: inner_parts,
    };

    EntityModelDef {
        texture_path: texture_path.to_string(),
        texture_size,
        parts: vec![root],
        is_opaque: !has_hat,
    }
}

/// Build a player skull model with a custom texture path.
pub(crate) fn player_skull_model(texture_path: &str) -> EntityModelDef {
    let head = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -8.0, -4.0],
            dimensions: [8.0, 8.0, 8.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    // Hat overlay always present for player heads
    let hat = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -8.0, -4.0],
            dimensions: [8.0, 8.0, 8.0],
            tex_offset: [32, 0],
            inflate: 0.25,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 0.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children: vec![head, hat],
    };

    EntityModelDef {
        texture_path: texture_path.to_string(),
        texture_size: [64, 64],
        parts: vec![root],
        is_opaque: false, // hat layer is transparent
    }
}

/// Decode a hex-encoded PNG skin into TextureData.
pub(crate) fn decode_hex_skin(hex_str: &str) -> Option<TextureData> {
    // Decode hex string to bytes
    let hex_clean = hex_str.trim();
    if hex_clean.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(hex_clean.len() / 2);
    for i in (0..hex_clean.len()).step_by(2) {
        let byte = u8::from_str_radix(&hex_clean[i..i + 2], 16).ok()?;
        bytes.push(byte);
    }

    png_bytes_to_texture(&bytes)
}

/// Decode a base64-encoded PNG skin into TextureData.
///
/// Accepts both standard (+/) and URL-safe (-_) alphabets, with or without
/// padding. This is the intended path for WASM/JS callers: resolve a username
/// to a skin PNG in JavaScript (via Mojang's session API), encode as base64,
/// and pass it through the `skin_base64` block property.
pub(crate) fn decode_base64_skin(b64: &str) -> Option<TextureData> {
    let bytes = decode_base64(b64)?;
    png_bytes_to_texture(&bytes)
}

fn png_bytes_to_texture(bytes: &[u8]) -> Option<TextureData> {
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.to_rgba8();
    Some(TextureData {
        width: rgba.width(),
        height: rgba.height(),
        pixels: rgba.into_raw(),
        is_animated: false,
        frame_count: 1,
        animation: None,
    })
}

/// Base64 decoder. Supports both `+/` and URL-safe `-_` alphabets; `=` padding
/// is tolerated but not required. Returns None on invalid characters.
fn decode_base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for b in s.bytes() {
        let v: u32 = match b {
            b'A'..=b'Z' => (b - b'A') as u32,
            b'a'..=b'z' => (b - b'a' + 26) as u32,
            b'0'..=b'9' => (b - b'0' + 52) as u32,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => continue,
            c if c.is_ascii_whitespace() => continue,
            _ => return None,
        };
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1u32 << bits).wrapping_sub(1);
        }
    }
    Some(out)
}

/// Determine player skin fallback path based on UUID.
/// Even first hex digit → Steve, odd → Alex.
pub(crate) fn player_skin_fallback_path(block: &InputBlock) -> &'static str {
    if let Some(uuid) = block.properties.get("uuid") {
        let first_char = uuid.chars().find(|c| c.is_ascii_hexdigit());
        if let Some(ch) = first_char {
            let digit = ch.to_digit(16).unwrap_or(0);
            if digit % 2 == 1 {
                return "entity/player/slim/alex";
            }
        }
    }
    "entity/player/wide/steve"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_player_head() {
        let block = InputBlock::new("minecraft:player_head");
        let entity = super::super::detect_block_entity(&block);
        assert!(matches!(entity, Some(super::super::BlockEntityType::Skull(SkullType::Player))));
    }

    #[test]
    fn test_detect_player_wall_head() {
        let block = InputBlock::new("minecraft:player_wall_head");
        let entity = super::super::detect_block_entity(&block);
        assert!(matches!(entity, Some(super::super::BlockEntityType::Skull(SkullType::Player))));
    }

    #[test]
    fn test_decode_hex_skin_invalid() {
        assert!(decode_hex_skin("not_hex").is_none());
        assert!(decode_hex_skin("abc").is_none()); // odd length
        assert!(decode_hex_skin("0000").is_none()); // valid hex but not a PNG
    }

    #[test]
    fn test_decode_hex_skin_valid() {
        // Create a minimal 1x1 PNG and hex-encode it
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(1, 1, Rgba([255, 0, 0, 255]));
        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
        let hex: String = buf.iter().map(|b| format!("{:02x}", b)).collect();

        let tex = decode_hex_skin(&hex).unwrap();
        assert_eq!(tex.width, 1);
        assert_eq!(tex.height, 1);
    }

    #[test]
    fn test_player_skin_fallback_steve() {
        let block = InputBlock::new("minecraft:player_head");
        assert_eq!(player_skin_fallback_path(&block), "entity/player/wide/steve");

        // Even first digit → Steve
        let block = InputBlock::new("minecraft:player_head")
            .with_property("uuid", "2a3b4c5d");
        assert_eq!(player_skin_fallback_path(&block), "entity/player/wide/steve");
    }

    #[test]
    fn test_player_skin_fallback_alex_by_uuid() {
        // Odd first digit → Alex
        let block = InputBlock::new("minecraft:player_head")
            .with_property("uuid", "1a2b3c4d");
        assert_eq!(player_skin_fallback_path(&block), "entity/player/slim/alex");
    }

    #[test]
    fn test_player_skull_geometry_count() {
        // Player skull: head (6 faces) + hat overlay (6 faces) = 12 faces
        let model = player_skull_model("test_texture");
        fn count_cubes(parts: &[super::super::EntityPart]) -> usize {
            let mut count = 0;
            for part in parts {
                count += part.cubes.len();
                count += count_cubes(&part.children);
            }
            count
        }
        let total_cubes = count_cubes(&model.parts);
        assert_eq!(total_cubes, 2); // head + hat
        // Each cube = 6 faces, so 12 faces total
        assert_eq!(total_cubes * 6, 12);
    }
}
