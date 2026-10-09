use crate::{Error, lz4};
use flate2::read::{GzDecoder, ZlibDecoder};
use lumilio_nbt::Tag;
use std::io::Read;

const SECTOR: u64 = 4096;
const HEADER: usize = 8192;
/// A chunk's decompressed NBT is bounded; real ones are well under 1 MiB.
const MAX_CHUNK: usize = 16 * 1024 * 1024;
/// Compression ids from the region format; 128 added means the data lives in
/// an external `c.<x>.<z>.mcc` file.
const EXTERNAL: u8 = 128;

/// Where a region's bytes come from.
pub trait Source {
    /// Up to `len` bytes from `offset` of the region file; a short read means
    /// the end of the file.
    fn read(&self, offset: u64, len: usize) -> Result<Vec<u8>, Error>;
    /// The whole external chunk file `name` (`c.<x>.<z>.mcc`, with absolute
    /// chunk coordinates) beside the region, or `None` if there is none.
    fn external(&self, name: &str) -> Result<Option<Vec<u8>>, Error>;
}

/// Where a chunk sits in the region file and when it was last saved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChunkLocation {
    pub offset: u64,
    pub sectors: u64,
    pub timestamp: u32,
}

/// One region file: 32 by 32 chunks.
pub struct Region<S> {
    source: S,
    header: Vec<u8>,
    /// The region's coordinates, which name its external chunk files.
    x: i32,
    z: i32,
}

impl<S: Source> Region<S> {
    /// Reads the 8 KiB header. An empty file is a region with no chunks (the
    /// game leaves those behind); any other file shorter than the header is
    /// not a region.
    pub fn open(source: S, region_x: i32, region_z: i32) -> Result<Self, Error> {
        let mut header = source.read(0, HEADER)?;
        if header.is_empty() {
            header = vec![0; HEADER];
        }
        if header.len() < HEADER {
            return Err(Error::Format("region header is truncated"));
        }
        Ok(Self {
            source,
            header,
            x: region_x,
            z: region_z,
        })
    }

    fn word(&self, at: usize) -> u32 {
        u32::from_be_bytes([
            self.header[at],
            self.header[at + 1],
            self.header[at + 2],
            self.header[at + 3],
        ])
    }

    /// Where chunk `(x, z)` of this region (each 0..32) is, if it was ever
    /// generated.
    pub fn location(&self, x: usize, z: usize) -> Option<ChunkLocation> {
        if x >= 32 || z >= 32 {
            return None;
        }
        let at = (z * 32 + x) * 4;
        let entry = self.word(at);
        let (offset, sectors) = (u64::from(entry >> 8), u64::from(entry & 0xff));
        // Sector 0 and 1 are the header; an entry pointing into it is empty.
        (offset >= 2 && sectors > 0).then(|| ChunkLocation {
            offset,
            sectors,
            timestamp: self.word(SECTOR as usize + at),
        })
    }

    /// How many of the 1024 chunks exist.
    pub fn chunk_count(&self) -> usize {
        (0..1024)
            .filter(|index| self.location(index % 32, index / 32).is_some())
            .count()
    }

    /// The NBT of chunk `(x, z)`, or `None` when it does not exist.
    pub fn chunk(&self, x: usize, z: usize) -> Result<Option<Tag>, Error> {
        let Some(location) = self.location(x, z) else {
            return Ok(None);
        };
        let head = self.source.read(location.offset * SECTOR, 5)?;
        if head.len() < 5 {
            return Err(Error::Format("chunk header is truncated"));
        }
        let length = u32::from_be_bytes([head[0], head[1], head[2], head[3]]) as usize;
        let kind = head[4];
        if length == 0 || length > (location.sectors * SECTOR) as usize {
            return Err(Error::Format("chunk length does not fit its sectors"));
        }
        let packed = if kind & EXTERNAL != 0 {
            let name = format!(
                "c.{}.{}.mcc",
                self.x * 32 + x as i32,
                self.z * 32 + z as i32
            );
            self.source
                .external(&name)?
                .ok_or(Error::Format("external chunk file is missing"))?
        } else {
            let body = self.source.read(location.offset * SECTOR + 5, length - 1)?;
            if body.len() < length - 1 {
                return Err(Error::Format("chunk data is truncated"));
            }
            body
        };
        let bytes = decompress(kind & !EXTERNAL, &packed)?;
        lumilio_nbt::parse(&bytes)
            .map(Some)
            .map_err(|error| Error::Data(error.to_string()))
    }
}

fn decompress(kind: u8, packed: &[u8]) -> Result<Vec<u8>, Error> {
    let inflate = |mut reader: Box<dyn Read + '_>| {
        let mut out = Vec::new();
        reader
            .by_ref()
            .take(MAX_CHUNK as u64 + 1)
            .read_to_end(&mut out)
            .map_err(|error| Error::Data(error.to_string()))?;
        if out.len() > MAX_CHUNK {
            return Err(Error::Format("chunk is too large"));
        }
        Ok(out)
    };
    match kind {
        1 => inflate(Box::new(GzDecoder::new(packed))),
        2 => inflate(Box::new(ZlibDecoder::new(packed))),
        3 => {
            if packed.len() > MAX_CHUNK {
                return Err(Error::Format("chunk is too large"));
            }
            Ok(packed.to_vec())
        }
        4 => lz4::decode(packed, MAX_CHUNK),
        other => Err(Error::Compression(other)),
    }
}

/// The region coordinates in a region file name, `r.<x>.<z>.mca`.
pub fn region_coords(name: &str) -> Option<(i32, i32)> {
    let mut parts = name.strip_prefix("r.")?.strip_suffix(".mca")?.split('.');
    let x = parts.next()?.parse().ok()?;
    let z = parts.next()?.parse().ok()?;
    parts.next().is_none().then_some((x, z))
}
