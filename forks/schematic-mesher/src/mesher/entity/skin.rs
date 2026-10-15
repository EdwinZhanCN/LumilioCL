//! Player skins for player heads and players: a `skin_base64` or hex `skin`
//! property, else the default skin the game gives the `uuid`.
//!
//! From schematic-mesher's original `skull.rs` and `player.rs`
//! (AGPL-3.0-only, Schem-at); the head and body geometry is now the game's own
//! layers (LumilioCL local change, plan schematic-studio T7).

use crate::resource_pack::{ResourcePack, TextureData};
use crate::types::InputBlock;

/// `DefaultPlayerSkin`'s skins in its order, from 1.19.3 (LumilioCL local
/// change, plan schematic-studio T35; read from the 26.3 client).
const DEFAULT_SKINS: [&str; 18] = [
    "entity/player/slim/alex",
    "entity/player/slim/ari",
    "entity/player/slim/efe",
    "entity/player/slim/kai",
    "entity/player/slim/makena",
    "entity/player/slim/noor",
    "entity/player/slim/steve",
    "entity/player/slim/sunny",
    "entity/player/slim/zuri",
    "entity/player/wide/alex",
    "entity/player/wide/ari",
    "entity/player/wide/efe",
    "entity/player/wide/kai",
    "entity/player/wide/makena",
    "entity/player/wide/noor",
    "entity/player/wide/steve",
    "entity/player/wide/sunny",
    "entity/player/wide/zuri",
];

/// Java's `UUID.hashCode` of a UUID written with or without dashes.
fn uuid_hash(text: &str) -> Option<i32> {
    let hex: String = text.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 {
        return None;
    }
    let bits = u128::from_str_radix(&hex, 16).ok()?;
    let hilo = ((bits >> 64) as u64) ^ (bits as u64);
    Some(((hilo >> 32) as u32 ^ hilo as u32) as i32)
}

/// The decoded skin of `block`, if it carries one.
pub(crate) fn skin_of(block: &InputBlock) -> Option<TextureData> {
    block
        .properties
        .get("skin_base64")
        .and_then(|skin| decode_base64_skin(skin))
        .or_else(|| {
            block
                .properties
                .get("skin")
                .and_then(|skin| decode_hex_skin(skin))
        })
}

/// Slim arms: the `slim` property, or a slim default skin.
fn is_slim(block: &InputBlock, pack: &ResourcePack) -> bool {
    block.properties.get("slim").is_some_and(|v| v == "true")
        || player_skin_fallback_path(block, pack).contains("/slim/")
}

/// The player model layer name for `block`: `player` or `player_slim`.
pub(crate) fn player_model(block: &InputBlock, pack: &ResourcePack) -> &'static str {
    if is_slim(block, pack) {
        "player_slim"
    } else {
        "player"
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

/// The default skin of `block`'s `uuid`, as `DefaultPlayerSkin.get` picks
/// it: one of eighteen by the UUID's hash where the version has them, else
/// (before 1.19.3) Alex for an odd hash and Steve for an even one. A short
/// id keeps the parity of its first digit; none is Steve.
pub(crate) fn player_skin_fallback_path(block: &InputBlock, pack: &ResourcePack) -> &'static str {
    if let Some(hash) = block
        .properties
        .get("uuid")
        .and_then(|uuid| uuid_hash(uuid))
    {
        if pack.get_texture(DEFAULT_SKINS[1]).is_some() {
            return DEFAULT_SKINS[hash.rem_euclid(18) as usize];
        }
        return if hash & 1 == 1 {
            "entity/player/slim/alex"
        } else {
            "entity/player/wide/steve"
        };
    }
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
    fn test_decode_hex_skin_invalid() {
        assert!(decode_hex_skin("not_hex").is_none());
        assert!(decode_hex_skin("abc").is_none()); // odd length
        assert!(decode_hex_skin("0000").is_none()); // valid hex but not a PNG
    }

    #[test]
    fn test_player_skin_fallback_by_uuid() {
        let pack = ResourcePack::new();
        let block = InputBlock::new("minecraft:player_head").with_property("uuid", "2a3b4c5d");
        assert_eq!(
            player_skin_fallback_path(&block, &pack),
            "entity/player/wide/steve"
        );
        let block = InputBlock::new("minecraft:player_head").with_property("uuid", "1a2b3c4d");
        assert_eq!(
            player_skin_fallback_path(&block, &pack),
            "entity/player/slim/alex"
        );
        assert_eq!(player_model(&block, &pack), "player_slim");
    }

    /// T35: a full UUID picks the game's default skin by its hash; the NIL
    /// UUID (a mannequin without a profile) is the first, slim Alex. Before
    /// the eighteen skins, the hash's parity picks Steve or Alex.
    #[test]
    fn a_uuid_picks_the_game_s_default_skin() {
        let mut pack = ResourcePack::new();
        let nil = InputBlock::new("entity:mannequin")
            .with_property("uuid", "00000000-0000-0000-0000-000000000000");
        // Java: new UUID(0, 7).hashCode() == 7, and skin 7 is slim Sunny.
        let seven = InputBlock::new("entity:player")
            .with_property("uuid", "00000000-0000-0000-0000-000000000007");
        assert_eq!(
            player_skin_fallback_path(&nil, &pack),
            "entity/player/wide/steve"
        );
        assert_eq!(
            player_skin_fallback_path(&seven, &pack),
            "entity/player/slim/alex"
        );
        pack.add_texture(
            "minecraft",
            DEFAULT_SKINS[1],
            TextureData::new(1, 1, vec![0; 4]),
        );
        assert_eq!(
            player_skin_fallback_path(&nil, &pack),
            "entity/player/slim/alex"
        );
        assert_eq!(
            player_skin_fallback_path(&seven, &pack),
            "entity/player/slim/sunny"
        );
        assert_eq!(player_model(&nil, &pack), "player_slim");
    }
}
