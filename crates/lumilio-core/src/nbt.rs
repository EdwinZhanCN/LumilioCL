//! A small reader for the game's NBT format (as found in `level.dat`).
//!
//! Only reading is needed. Input is bounded: the decompressed size and the
//! nesting depth are capped so a hostile file cannot exhaust memory or stack.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::Read;

use flate2::read::GzDecoder;

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
        Ok(String::from_utf8_lossy(self.take(length)?).into_owned())
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
    } else {
        parse(data)
    }
}

/// Builds documents for tests here and in other modules.
#[cfg(test)]
pub(crate) mod build {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    pub fn name(out: &mut Vec<u8>, text: &str) {
        out.extend((text.len() as u16).to_be_bytes());
        out.extend(text.as_bytes());
    }

    pub fn string(out: &mut Vec<u8>, key: &str, value: &str) {
        out.push(8);
        name(out, key);
        name(out, value);
    }

    pub fn long(out: &mut Vec<u8>, key: &str, value: i64) {
        out.push(4);
        name(out, key);
        out.extend(value.to_be_bytes());
    }

    pub fn byte(out: &mut Vec<u8>, key: &str, value: i8) {
        out.push(1);
        name(out, key);
        out.push(value as u8);
    }

    pub fn compound_start(out: &mut Vec<u8>, key: &str) {
        out.push(10);
        name(out, key);
    }

    pub fn end(out: &mut Vec<u8>) {
        out.push(0);
    }

    pub fn gzip(data: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::build::*;
    use super::*;

    fn sample() -> Vec<u8> {
        let mut out = Vec::new();
        compound_start(&mut out, "");
        compound_start(&mut out, "Data");
        string(&mut out, "LevelName", "My World");
        long(&mut out, "LastPlayed", 1_700_000_000_000);
        byte(&mut out, "hardcore", 1);
        compound_start(&mut out, "Version");
        string(&mut out, "Name", "1.21.1");
        end(&mut out);
        end(&mut out);
        end(&mut out);
        out
    }

    #[test]
    fn parses_nested_compounds() {
        let tag = parse(&sample()).unwrap();
        assert_eq!(
            tag.at(&["Data", "LevelName"]).unwrap().as_str(),
            Some("My World")
        );
        assert_eq!(
            tag.at(&["Data", "LastPlayed"]).unwrap().as_i64(),
            Some(1_700_000_000_000)
        );
        assert_eq!(tag.at(&["Data", "hardcore"]).unwrap().as_i64(), Some(1));
        assert_eq!(
            tag.at(&["Data", "Version", "Name"]).unwrap().as_str(),
            Some("1.21.1")
        );
        assert!(tag.at(&["Data", "missing"]).is_none());
    }

    #[test]
    fn reads_gzip_and_plain_documents() {
        let plain = sample();
        assert_eq!(
            parse_maybe_gzip(&gzip(&plain)).unwrap(),
            parse(&plain).unwrap()
        );
        assert!(parse_maybe_gzip(&plain).is_ok());
    }

    #[test]
    fn rejects_malformed_input_without_panicking() {
        let data = sample();
        for cut in 0..data.len() {
            assert!(parse(&data[..cut]).is_err(), "truncated at {cut}");
        }
        assert_eq!(parse(&[1, 0, 0]), Err(NbtError::NotACompound));
        // Unknown child tag id.
        let mut bad = Vec::new();
        compound_start(&mut bad, "");
        bad.push(99);
        name(&mut bad, "x");
        assert_eq!(parse(&bad), Err(NbtError::BadTag(99)));
    }

    #[test]
    fn deep_nesting_is_refused() {
        let mut out = Vec::new();
        compound_start(&mut out, "");
        for _ in 0..(MAX_DEPTH + 10) {
            compound_start(&mut out, "n");
        }
        assert_eq!(parse(&out), Err(NbtError::TooDeep));
    }

    #[test]
    fn a_huge_declared_list_does_not_allocate_up_front() {
        let mut out = Vec::new();
        compound_start(&mut out, "");
        out.push(9);
        name(&mut out, "l");
        out.push(3);
        out.extend(i32::MAX.to_be_bytes());
        assert_eq!(parse(&out), Err(NbtError::Truncated));
    }
}
