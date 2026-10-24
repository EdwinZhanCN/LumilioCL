//! CurseForge's file fingerprint: 32-bit MurmurHash2 with seed 1 over the
//! file's bytes, leaving out every tab, line feed, carriage return and space.
//! MurmurHash2 is Austin Appleby's public-domain algorithm.

use std::fs::File;
use std::io;
use std::io::{BufReader, Read};
use std::path::Path;

const M: u32 = 0x5bd1_e995;
const SEED: u32 = 1;

const fn counted(byte: u8) -> bool {
    !matches!(byte, b'\t' | b'\n' | b'\r' | b' ')
}

/// MurmurHash2 fed one byte at a time; it needs the total length first.
struct Murmur2 {
    hash: u32,
    block: [u8; 4],
    filled: usize,
}

impl Murmur2 {
    fn new(length: u64) -> Self {
        // The algorithm mixes in the length as 32 bits.
        #[allow(clippy::cast_possible_truncation)]
        let length = length as u32;
        Self {
            hash: SEED ^ length,
            block: [0; 4],
            filled: 0,
        }
    }

    fn push(&mut self, byte: u8) {
        self.block[self.filled] = byte;
        self.filled += 1;
        if self.filled == 4 {
            let mut k = u32::from_le_bytes(self.block);
            k = k.wrapping_mul(M);
            k ^= k >> 24;
            k = k.wrapping_mul(M);
            self.hash = self.hash.wrapping_mul(M) ^ k;
            self.filled = 0;
        }
    }

    fn finish(mut self) -> u32 {
        let tail = self.block;
        if self.filled == 3 {
            self.hash ^= u32::from(tail[2]) << 16;
        }
        if self.filled >= 2 {
            self.hash ^= u32::from(tail[1]) << 8;
        }
        if self.filled >= 1 {
            self.hash ^= u32::from(tail[0]);
            self.hash = self.hash.wrapping_mul(M);
        }
        self.hash ^= self.hash >> 13;
        self.hash = self.hash.wrapping_mul(M);
        self.hash ^ (self.hash >> 15)
    }
}

/// The fingerprint of bytes already in memory.
#[cfg(test)]
#[must_use]
pub fn of_bytes(bytes: &[u8]) -> u32 {
    let length = bytes.iter().filter(|byte| counted(**byte)).count() as u64;
    let mut murmur = Murmur2::new(length);
    for byte in bytes.iter().copied().filter(|byte| counted(*byte)) {
        murmur.push(byte);
    }
    murmur.finish()
}

fn each_counted(path: &Path, mut visit: impl FnMut(u8)) -> io::Result<()> {
    let mut reader = BufReader::with_capacity(64 * 1024, File::open(path)?);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            return Ok(());
        }
        for byte in buffer[..read].iter().copied().filter(|byte| counted(*byte)) {
            visit(byte);
        }
    }
}

/// The fingerprint of a file, read twice (length, then hash) so a large
/// resource pack is never held in memory.
pub fn of_file(path: &Path) -> io::Result<u32> {
    let mut length = 0_u64;
    each_counted(path, |_| length += 1)?;
    let mut murmur = Murmur2::new(length);
    each_counted(path, |byte| murmur.push(byte))?;
    Ok(murmur.finish())
}

#[cfg(test)]
mod tests;
