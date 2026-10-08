//! A small reader and writer for the game's NBT format (`level.dat`,
//! `servers.dat`).
//!
//! Input is bounded: the decompressed size and the nesting depth are capped so
//! a hostile file cannot exhaust memory or stack. Strings use the game's
//! "modified UTF-8", so text with emoji survives a read and write.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::Read;

use flate2::read::{GzDecoder, ZlibDecoder};

/// Largest decompressed document accepted.
pub const MAX_DOCUMENT: u64 = 64 * 1024 * 1024;
const MAX_DEPTH: usize = 128;

#[derive(Clone, Debug, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List(Vec<Tag>),
    Compound(BTreeMap<String, Tag>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl Tag {
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Tag> {
        match self {
            Self::Compound(map) => map.get(key),
            _ => None,
        }
    }

    /// Follows `path` through nested compounds.
    #[must_use]
    pub fn at(&self, path: &[&str]) -> Option<&Tag> {
        path.iter().try_fold(self, |tag, key| tag.get(key))
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }

    /// Any integer tag as `i64`.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Byte(v) => Some(i64::from(*v)),
            Self::Short(v) => Some(i64::from(*v)),
            Self::Int(v) => Some(i64::from(*v)),
            Self::Long(v) => Some(*v),
            _ => None,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum NbtError {
    Truncated,
    BadTag(u8),
    TooDeep,
    TooLarge,
    /// The root of a document must be a compound.
    NotACompound,
}

impl Display for NbtError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => f.write_str("data ends unexpectedly"),
            Self::BadTag(id) => write!(f, "unknown tag type {id}"),
            Self::TooDeep => f.write_str("nesting is too deep"),
            Self::TooLarge => f.write_str("document is too large"),
            Self::NotACompound => f.write_str("root is not a compound"),
        }
    }
}

impl Error for NbtError {}

struct Reader<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], NbtError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(NbtError::Truncated)?;
        let slice = self
            .data
            .get(self.position..end)
            .ok_or(NbtError::Truncated)?;
        self.position = end;
        Ok(slice)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], NbtError> {
        Ok(self
            .take(N)?
            .try_into()
            .expect("take returns exactly N bytes"))
    }

    fn length(&mut self) -> Result<usize, NbtError> {
        let value = i32::from_be_bytes(self.array()?);
        // A negative length is treated as empty, as the game does.
        Ok(usize::try_from(value).unwrap_or(0))
    }

    fn string(&mut self) -> Result<String, NbtError> {
        let length = usize::from(u16::from_be_bytes(self.array()?));
        Ok(decode_modified_utf8(self.take(length)?))
    }

    fn payload(&mut self, id: u8, depth: usize) -> Result<Tag, NbtError> {
        if depth > MAX_DEPTH {
            return Err(NbtError::TooDeep);
        }
        Ok(match id {
            1 => Tag::Byte(i8::from_be_bytes(self.array()?)),
            2 => Tag::Short(i16::from_be_bytes(self.array()?)),
            3 => Tag::Int(i32::from_be_bytes(self.array()?)),
            4 => Tag::Long(i64::from_be_bytes(self.array()?)),
            5 => Tag::Float(f32::from_be_bytes(self.array()?)),
            6 => Tag::Double(f64::from_be_bytes(self.array()?)),
            7 => {
                let length = self.length()?;
                Tag::ByteArray(self.take(length)?.iter().map(|b| *b as i8).collect())
            }
            8 => Tag::String(self.string()?),
            9 => {
                let element = u8::from_be_bytes(self.array()?);
                let length = self.length()?;
                let mut items = Vec::new();
                for _ in 0..length {
                    items.push(self.payload(element, depth + 1)?);
                }
                Tag::List(items)
            }
            10 => {
                let mut map = BTreeMap::new();
                loop {
                    let child = u8::from_be_bytes(self.array()?);
                    if child == 0 {
                        break;
                    }
                    let name = self.string()?;
                    map.insert(name, self.payload(child, depth + 1)?);
                }
                Tag::Compound(map)
            }
            11 => {
                let length = self.length()?;
                let mut values = Vec::new();
                for _ in 0..length {
                    values.push(i32::from_be_bytes(self.array()?));
                }
                Tag::IntArray(values)
            }
            12 => {
                let length = self.length()?;
                let mut values = Vec::new();
                for _ in 0..length {
                    values.push(i64::from_be_bytes(self.array()?));
                }
                Tag::LongArray(values)
            }
            other => return Err(NbtError::BadTag(other)),
        })
    }
}

/// Decodes Java's modified UTF-8 (NUL as `C0 80`, supplementary characters as
/// two three-byte surrogates). Malformed bytes become U+FFFD.
fn decode_modified_utf8(bytes: &[u8]) -> String {
    if bytes.is_ascii() {
        // Plain ASCII is identical in both encodings.
        return String::from_utf8_lossy(bytes).into_owned();
    }
    let mut units: Vec<u16> = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let first = bytes[at];
        let (unit, used) = match first {
            0x00..=0x7f => (u16::from(first), 1),
            0xc0..=0xdf if at + 1 < bytes.len() => (
                u16::from(first & 0x1f) << 6 | u16::from(bytes[at + 1] & 0x3f),
                2,
            ),
            0xe0..=0xef if at + 2 < bytes.len() => (
                u16::from(first & 0x0f) << 12
                    | u16::from(bytes[at + 1] & 0x3f) << 6
                    | u16::from(bytes[at + 2] & 0x3f),
                3,
            ),
            _ => (0xfffd, 1),
        };
        units.push(unit);
        at += used;
    }
    String::from_utf16_lossy(&units)
}

fn encode_modified_utf8(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    for unit in text.encode_utf16() {
        match unit {
            0x0001..=0x007f => out.push(unit as u8),
            0x0000 | 0x0080..=0x07ff => {
                out.push(0xc0 | (unit >> 6) as u8);
                out.push(0x80 | (unit & 0x3f) as u8);
            }
            _ => {
                out.push(0xe0 | (unit >> 12) as u8);
                out.push(0x80 | ((unit >> 6) & 0x3f) as u8);
                out.push(0x80 | (unit & 0x3f) as u8);
            }
        }
    }
    out
}

fn write_name(out: &mut Vec<u8>, text: &str) {
    let bytes = encode_modified_utf8(text);
    // A name or string longer than the format allows is cut, never wrapped.
    let length = u16::try_from(bytes.len()).unwrap_or(u16::MAX);
    out.extend(length.to_be_bytes());
    out.extend(&bytes[..usize::from(length)]);
}

fn tag_id(tag: &Tag) -> u8 {
    match tag {
        Tag::Byte(_) => 1,
        Tag::Short(_) => 2,
        Tag::Int(_) => 3,
        Tag::Long(_) => 4,
        Tag::Float(_) => 5,
        Tag::Double(_) => 6,
        Tag::ByteArray(_) => 7,
        Tag::String(_) => 8,
        Tag::List(_) => 9,
        Tag::Compound(_) => 10,
        Tag::IntArray(_) => 11,
        Tag::LongArray(_) => 12,
    }
}

fn write_length(out: &mut Vec<u8>, length: usize) {
    out.extend(i32::try_from(length).unwrap_or(i32::MAX).to_be_bytes());
}

fn write_payload(out: &mut Vec<u8>, tag: &Tag) {
    match tag {
        Tag::Byte(v) => out.push(*v as u8),
        Tag::Short(v) => out.extend(v.to_be_bytes()),
        Tag::Int(v) => out.extend(v.to_be_bytes()),
        Tag::Long(v) => out.extend(v.to_be_bytes()),
        Tag::Float(v) => out.extend(v.to_be_bytes()),
        Tag::Double(v) => out.extend(v.to_be_bytes()),
        Tag::ByteArray(values) => {
            write_length(out, values.len());
            out.extend(values.iter().map(|b| *b as u8));
        }
        Tag::String(text) => write_name(out, text),
        Tag::List(items) => {
            out.push(items.first().map_or(0, tag_id));
            write_length(out, items.len());
            for item in items {
                write_payload(out, item);
            }
        }
        Tag::Compound(map) => {
            for (name, child) in map {
                out.push(tag_id(child));
                write_name(out, name);
                write_payload(out, child);
            }
            out.push(0);
        }
        Tag::IntArray(values) => {
            write_length(out, values.len());
            for value in values {
                out.extend(value.to_be_bytes());
            }
        }
        Tag::LongArray(values) => {
            write_length(out, values.len());
            for value in values {
                out.extend(value.to_be_bytes());
            }
        }
    }
}

/// Serialises a compound as an uncompressed document with an empty root name,
/// the form `servers.dat` uses. Anything else yields an empty compound.
#[must_use]
pub fn to_bytes(root: &Tag) -> Vec<u8> {
    let empty = Tag::Compound(BTreeMap::new());
    let root = if matches!(root, Tag::Compound(_)) {
        root
    } else {
        &empty
    };
    let mut out = vec![10];
    write_name(&mut out, "");
    write_payload(&mut out, root);
    out
}

/// Parses an uncompressed NBT document whose root is a compound. The root's
/// own name is discarded.
pub fn parse(data: &[u8]) -> Result<Tag, NbtError> {
    let mut reader = Reader { data, position: 0 };
    let id = u8::from_be_bytes(reader.array()?);
    if id != 10 {
        return Err(NbtError::NotACompound);
    }
    let _root_name = reader.string()?;
    reader.payload(10, 0)
}

/// Parses a gzip-compressed document, or an uncompressed one (some tools write
/// `level.dat` without compression).
pub fn parse_maybe_gzip(data: &[u8]) -> Result<Tag, NbtError> {
    if data.starts_with(&[0x1f, 0x8b]) {
        let mut inflated = Vec::new();
        GzDecoder::new(data)
            .take(MAX_DOCUMENT + 1)
            .read_to_end(&mut inflated)
            .map_err(|_| NbtError::Truncated)?;
        if inflated.len() as u64 > MAX_DOCUMENT {
            return Err(NbtError::TooLarge);
        }
        parse(&inflated)
    } else if data.first().is_some_and(|byte| byte & 0x0f == 8)
        && data
            .get(1)
            .is_some_and(|byte| (u16::from(data[0]) * 256 + u16::from(*byte)) % 31 == 0)
    {
        parse_zlib(data)
    } else {
        parse(data)
    }
}

pub fn parse_zlib(data: &[u8]) -> Result<Tag, NbtError> {
    let mut inflated = Vec::new();
    ZlibDecoder::new(data)
        .take(MAX_DOCUMENT + 1)
        .read_to_end(&mut inflated)
        .map_err(|_| NbtError::Truncated)?;
    if inflated.len() as u64 > MAX_DOCUMENT {
        return Err(NbtError::TooLarge);
    }
    parse(&inflated)
}

#[cfg(test)]
mod tests;
