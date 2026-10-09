//! Regions written by the vanilla server, trimmed to 4×4 chunks; how they were
//! made is in `tests/data/README.md`.
use crate::{Chunk, Error, Heightmap, Region, Source, columns, region_coords};
use std::path::PathBuf;

struct File(Vec<u8>);

impl Source for File {
    fn read(&self, offset: u64, len: usize) -> Result<Vec<u8>, Error> {
        let start = (offset as usize).min(self.0.len());
        Ok(self.0[start..(start + len).min(self.0.len())].to_vec())
    }
    fn external(&self, _: &str) -> Result<Option<Vec<u8>>, Error> {
        Ok(None)
    }
}

/// (version, region file, data version, compression id the server used).
const SAMPLES: [(&str, &str, i32, u8); 3] = [
    ("1.16.5", "r.0.-1.mca", 2586, 2),
    ("1.18.2", "r.0.0.mca", 2975, 2),
    // Written with `region-file-compression=lz4`.
    ("26.3", "r.0.0.mca", 5023, 4),
];

fn open(version: &str, name: &str) -> (Region<File>, Vec<u8>) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(version)
        .join(name);
    let bytes = std::fs::read(&path).unwrap();
    let (x, z) = region_coords(name).unwrap();
    (Region::open(File(bytes.clone()), x, z).unwrap(), bytes)
}

fn chunks(region: &Region<File>) -> Vec<((usize, usize), Chunk)> {
    (0..1024)
        .filter_map(|index| {
            let (x, z) = (index % 32, index / 32);
            let tag = region.chunk(x, z).unwrap()?;
            Some(((x, z), Chunk::from_nbt(&tag).unwrap()))
        })
        .collect()
}

/// The highest non-air block of a column, found by looking at every block.
fn highest(chunk: &Chunk, x: usize, z: usize) -> Option<i32> {
    chunk.sections.iter().find_map(|section| {
        (0..16)
            .rev()
            .find(|&y| !crate::chunk::is_air(section.block(x, y, z)))
            .map(|y| section.y * 16 + y as i32)
    })
}

#[test]
fn real_regions_of_three_versions_decode() {
    for (version, name, data_version, compression) in SAMPLES {
        let (region, bytes) = open(version, name);
        assert_eq!(region.chunk_count(), 16, "{version}");
        let first = (0..1024)
            .find_map(|index| region.location(index % 32, index / 32))
            .unwrap();
        assert_eq!(
            bytes[(first.offset * 4096 + 4) as usize],
            compression,
            "{version} compression"
        );
        let chunks = chunks(&region);
        assert_eq!(chunks.len(), 16);
        for (at, chunk) in &chunks {
            assert_eq!(chunk.data_version, data_version, "{version} {at:?}");
            let min_y = if version == "1.16.5" { 0 } else { -64 };
            assert_eq!(chunk.min_y, min_y, "{version} {at:?}");
        }
    }
}

#[test]
fn top_blocks_agree_with_the_games_own_heightmaps() {
    for (version, name, ..) in SAMPLES {
        let (region, _) = open(version, name);
        let mut checked = 0;
        for (at, chunk) in chunks(&region) {
            if !chunk.is_complete() {
                continue;
            }
            let surface = chunk.heightmap(Heightmap::WorldSurface).unwrap();
            let blocking = chunk.heightmap(Heightmap::MotionBlocking).unwrap();
            let found = columns(&chunk, |_| false);
            for index in 0..256 {
                let (x, z) = (index % 16, index / 16);
                let top = highest(&chunk, x, z);
                assert_eq!(surface[index], top, "{version} {at:?} column {index}");
                assert_eq!(found[index].map(|column| column.y), top);
                assert!(blocking[index] <= surface[index]);
                let biome = found[index].and_then(|column| column.biome).unwrap();
                assert!(biome.starts_with("minecraft:"), "{version} {biome}");
                checked += 1;
            }
        }
        assert!(checked >= 8 * 256, "{version}: {checked} columns");
    }
}

#[test]
fn known_columns_of_each_sample() {
    // (version, file, chunk in region, block, top y, biome) read off the
    // samples once and pinned; the 26.3 leaf litter has block properties, so
    // its palette holds `{id, properties}` beside `{"": name}` entries.
    for (version, name, (cx, cz), block, y, biome) in [
        (
            "1.16.5",
            "r.0.-1.mca",
            (8, 16),
            "minecraft:dark_oak_leaves",
            70,
            "minecraft:dark_forest",
        ),
        (
            "1.16.5",
            "r.0.-1.mca",
            (10, 16),
            "minecraft:grass_block",
            66,
            "minecraft:plains",
        ),
        (
            "1.18.2",
            "r.0.0.mca",
            (3, 0),
            "minecraft:grass_block",
            89,
            "minecraft:forest",
        ),
        (
            "26.3",
            "r.0.0.mca",
            (5, 0),
            "minecraft:grass_block",
            88,
            "minecraft:forest",
        ),
        (
            "26.3",
            "r.0.0.mca",
            (7, 3),
            "minecraft:leaf_litter",
            94,
            "minecraft:forest",
        ),
    ] {
        let (region, _) = open(version, name);
        let chunk = Chunk::from_nbt(&region.chunk(cx, cz).unwrap().unwrap()).unwrap();
        let column = columns(&chunk, |_| false)[0].unwrap();
        assert_eq!(
            (column.block, column.y, column.biome),
            (block, y, Some(biome)),
            "{version} chunk {cx},{cz}"
        );
    }
}

#[test]
fn chunks_the_server_had_not_finished_have_no_surface() {
    // Column x = 8 of the 26.3 sample was only partly generated.
    let (region, _) = open("26.3", "r.0.0.mca");
    let unfinished: Vec<_> = chunks(&region)
        .into_iter()
        .filter(|(_, chunk)| !chunk.is_complete())
        .map(|(at, chunk)| (at, chunk.heightmap(Heightmap::WorldSurface)))
        .collect();
    assert!(unfinished.len() >= 4);
    assert!(
        unfinished
            .iter()
            .any(|(at, heights)| at.0 == 8 && heights.is_none())
    );
}
