//! The block framing Minecraft uses for LZ4 chunks: the Java `lz4-java`
//! `LZ4BlockOutputStream` format, a sequence of blocks each with a 21-byte
//! header, ended by a block that decompresses to nothing.
//!
//! Header: the 8 bytes `LZ4Block`, a token (the high nibble 0x10 for stored,
//! 0x20 for LZ4), the compressed length, the decompressed length and a
//! checksum (little-endian 32-bit each). The checksum is of a hash the game
//! computes with a seeded xxHash; the data is validated by its lengths and by
//! the chunk's own NBT parse, so the checksum is read and not verified.
use crate::Error;

const MAGIC: &[u8; 8] = b"LZ4Block";
const HEADER: usize = 21;
/// A block in this format is at most 32 MiB; chunks are far smaller.
const MAX_BLOCK: usize = 32 * 1024 * 1024;

pub(crate) fn decode(data: &[u8], limit: usize) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    let mut at = 0;
    loop {
        let header = data
            .get(at..at + HEADER)
            .ok_or(Error::Format("truncated lz4 block header"))?;
        if &header[..8] != MAGIC {
            return Err(Error::Format("bad lz4 block magic"));
        }
        let method = header[8] & 0xf0;
        let word = |from: usize| {
            u32::from_le_bytes([
                header[from],
                header[from + 1],
                header[from + 2],
                header[from + 3],
            ]) as usize
        };
        let (compressed, plain) = (word(9), word(13));
        at += HEADER;
        if compressed > MAX_BLOCK || plain > MAX_BLOCK || out.len() + plain > limit {
            return Err(Error::Format("lz4 block too large"));
        }
        if plain == 0 {
            return Ok(out);
        }
        let body = data
            .get(at..at + compressed)
            .ok_or(Error::Format("truncated lz4 block"))?;
        at += compressed;
        match method {
            0x10 if compressed == plain => out.extend_from_slice(body),
            0x20 => out.extend(
                lz4_flex::block::decompress(body, plain)
                    .map_err(|error| Error::Data(error.to_string()))?,
            ),
            _ => return Err(Error::Format("unknown lz4 block method")),
        }
    }
}

#[cfg(test)]
pub(crate) fn encode(plain: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for part in plain.chunks(1 << 15) {
        let packed = lz4_flex::block::compress(part);
        out.extend_from_slice(MAGIC);
        out.push(0x20 | 0x07);
        out.extend((packed.len() as u32).to_le_bytes());
        out.extend((part.len() as u32).to_le_bytes());
        out.extend(0u32.to_le_bytes());
        out.extend(packed);
    }
    out.extend_from_slice(MAGIC);
    out.push(0x20 | 0x07);
    out.extend(0u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out
}
