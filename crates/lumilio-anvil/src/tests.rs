use super::*;
use crate::chunk::pack;
use lumilio_nbt::Tag;
use std::collections::BTreeMap;
use std::io::Write;

fn compound(entries: Vec<(&str, Tag)>) -> Tag {
    Tag::Compound(
        entries
            .into_iter()
            .map(|(key, tag)| (key.to_owned(), tag))
            .collect::<BTreeMap<_, _>>(),
    )
}

fn palette(names: &[&str]) -> Tag {
    Tag::List(
        names
            .iter()
            .map(|name| compound(vec![("Name", Tag::String((*name).into()))]))
            .collect(),
    )
}

/// A 1.18-layout section with `fill(x, y, z)` choosing each block's palette index.
fn modern_section(
    y: i8,
    names: &[&str],
    fill: impl Fn(usize, usize, usize) -> u16,
    spanning: bool,
) -> Tag {
    let indices: Vec<u16> = (0..4096)
        .map(|at| fill(at % 16, at / 256, (at / 16) % 16))
        .collect();
    let mut block_states = vec![("palette", palette(names))];
    if names.len() > 1 {
        block_states.push((
            "data",
            Tag::LongArray(pack(&indices, names.len(), 4, spanning)),
        ));
    }
    compound(vec![
        ("Y", Tag::Byte(y)),
        ("block_states", compound(block_states)),
    ])
}

fn modern_chunk(version: i32, status: &str, sections: Vec<Tag>) -> Tag {
    compound(vec![
        ("DataVersion", Tag::Int(version)),
        ("Status", Tag::String(status.into())),
        ("sections", Tag::List(sections)),
    ])
}

const STONE: &str = "minecraft:stone";
const AIR: &str = "minecraft:air";
const WATER: &str = "minecraft:water";
const GRASS: &str = "minecraft:short_grass";

struct Memory {
    bytes: Vec<u8>,
    external: BTreeMap<String, Vec<u8>>,
}

impl Source for Memory {
    fn read(&self, offset: u64, len: usize) -> Result<Vec<u8>, Error> {
        let start = (offset as usize).min(self.bytes.len());
        let end = (start + len).min(self.bytes.len());
        Ok(self.bytes[start..end].to_vec())
    }
    fn external(&self, name: &str) -> Result<Option<Vec<u8>>, Error> {
        Ok(self.external.get(name).cloned())
    }
}

/// A region file holding `(x, z, compression, payload)` chunks, each in its own sectors.
fn region(chunks: &[(usize, usize, u8, Vec<u8>)]) -> Memory {
    let mut bytes = vec![0u8; 8192];
    for (x, z, kind, payload) in chunks {
        let sector = bytes.len() / 4096;
        let length = payload.len() + 1;
        let sectors = (length + 4).div_ceil(4096);
        bytes.extend((length as u32).to_be_bytes());
        bytes.push(*kind);
        bytes.extend(payload);
        bytes.resize((sector + sectors) * 4096, 0);
        let entry = ((sector as u32) << 8) | sectors as u32;
        let at = (z * 32 + x) * 4;
        bytes[at..at + 4].copy_from_slice(&entry.to_be_bytes());
        bytes[4096 + at..4096 + at + 4].copy_from_slice(&1_700_000_000u32.to_be_bytes());
    }
    Memory {
        bytes,
        external: BTreeMap::new(),
    }
}

fn zlib(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn sample() -> Tag {
    modern_chunk(
        3955,
        "minecraft:full",
        vec![modern_section(
            0,
            &[STONE, AIR],
            |_, y, _| u16::from(y >= 8),
            false,
        )],
    )
}

#[test]
fn every_compression_the_game_writes_gives_back_the_same_chunk() {
    let nbt = lumilio_nbt::to_bytes(&sample());
    // A chunk too big for the region file is stored beside it, flagged by +128.
    let mut file = region(&[
        (0, 0, 2, zlib(&nbt)),
        (1, 0, 1, gzip(&nbt)),
        (2, 0, 3, nbt.clone()),
        (3, 0, 4, crate::lz4::encode(&nbt)),
        (4, 0, 2 + 128, vec![0]),
    ]);
    file.external.insert("c.4.0.mcc".into(), zlib(&nbt));
    let region = Region::open(file, 0, 0).unwrap();
    assert_eq!(region.chunk_count(), 5);
    for x in 0..5 {
        assert_eq!(
            region.chunk(x, 0).unwrap().as_ref(),
            Some(&sample()),
            "chunk {x}"
        );
    }
    assert_eq!(region.chunk(5, 0).unwrap(), None, "never generated");
    assert_eq!(region.chunk(0, 1).unwrap(), None);
    assert_eq!(
        region.chunk(40, 0).unwrap(),
        None,
        "out of range is not a chunk"
    );
    assert_eq!(region.location(0, 0).unwrap().timestamp, 1_700_000_000);
    assert!(region.location(0, 0).unwrap().offset >= 2);
}

#[test]
fn external_chunks_use_absolute_coordinates() {
    let nbt = lumilio_nbt::to_bytes(&sample());
    let mut file = region(&[(3, 2, 2 + 128, vec![0])]);
    // Region (-1, 5): chunk (3, 2) is chunk (-29, 162) in the world.
    file.external.insert("c.-29.162.mcc".into(), zlib(&nbt));
    let region = Region::open(file, -1, 5).unwrap();
    assert_eq!(region.chunk(3, 2).unwrap(), Some(sample()));
    let missing = Region::open(self::region(&[(3, 2, 2 + 128, vec![0])]), -1, 5).unwrap();
    assert!(matches!(missing.chunk(3, 2), Err(Error::Format(_))));
}

#[test]
fn broken_regions_and_chunks_are_errors_for_that_chunk_only() {
    assert!(matches!(
        Region::open(
            Memory {
                bytes: vec![0; 100],
                external: BTreeMap::new()
            },
            0,
            0
        ),
        Err(Error::Format(_))
    ));
    let nbt = lumilio_nbt::to_bytes(&sample());
    let file = region(&[
        (0, 0, 2, zlib(&nbt)),
        (1, 0, 9, vec![1, 2, 3]),
        (2, 0, 2, vec![0xde, 0xad, 0xbe, 0xef]),
        (3, 0, 3, vec![1, 2, 3]),
    ]);
    let mut bytes = file.bytes.clone();
    // Chunk 4 claims more bytes than its sectors hold; chunk 5 points past the end.
    let wide = region(&[(4, 0, 2, zlib(&nbt))]);
    let at = (u32::from_be_bytes(wide.bytes[16..20].try_into().unwrap()) >> 8) as usize * 4096;
    let mut lying = wide.bytes[at..at + 4096].to_vec();
    lying[..4].copy_from_slice(&1_000_000u32.to_be_bytes());
    let sector = bytes.len() / 4096;
    bytes.extend(lying);
    bytes[16..20].copy_from_slice(&(((sector as u32) << 8) | 1).to_be_bytes());
    let past = bytes.len() as u32 / 4096 + 50;
    bytes[20..24].copy_from_slice(&((past << 8) | 1).to_be_bytes());
    let region = Region::open(
        Memory {
            bytes,
            external: BTreeMap::new(),
        },
        0,
        0,
    )
    .unwrap();
    assert_eq!(
        region.chunk(0, 0).unwrap(),
        Some(sample()),
        "the good chunk still reads"
    );
    assert_eq!(region.chunk(1, 0), Err(Error::Compression(9)));
    assert!(matches!(region.chunk(2, 0), Err(Error::Data(_))));
    assert!(matches!(region.chunk(3, 0), Err(Error::Data(_))), "not NBT");
    assert!(region.chunk(4, 0).is_err());
    assert!(region.chunk(5, 0).is_err(), "beyond the end of the file");
}

#[test]
fn an_oversized_chunk_is_refused_not_inflated() {
    // 20 MiB of zeros compresses to a few KiB; the cap stops it from being expanded.
    let bomb = zlib(&vec![0u8; 20 * 1024 * 1024]);
    let region = Region::open(region(&[(0, 0, 2, bomb)]), 0, 0).unwrap();
    assert_eq!(region.chunk(0, 0), Err(Error::Format("chunk is too large")));
}

#[test]
fn garbage_never_panics() {
    let mut seed = 0x1234_5678_u64;
    let mut next = move || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) as u8
    };
    for round in 0..40 {
        let mut bytes: Vec<u8> = (0..8192 + 20000).map(|_| next()).collect();
        if round % 2 == 0 {
            // Plausible sector entries so chunk reads actually go looking.
            for entry in 0..1024 {
                let sector = 2 + (entry % 5) as u32;
                bytes[entry * 4..entry * 4 + 4].copy_from_slice(&((sector << 8) | 2).to_be_bytes());
            }
        }
        let region = Region::open(
            Memory {
                bytes,
                external: BTreeMap::new(),
            },
            0,
            0,
        )
        .unwrap();
        for index in 0..1024 {
            let _ = region.chunk(index % 32, index / 32);
        }
    }
}

#[test]
fn packed_indices_round_trip_in_both_layouts() {
    for palette_len in [2, 5, 16, 17, 33, 100, 1000] {
        let indices: Vec<u16> = (0..4096)
            .map(|at| ((at * 7 + at / 3) % palette_len) as u16)
            .collect();
        let names: Vec<String> = (0..palette_len)
            .map(|n| format!("minecraft:b{n}"))
            .collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        for (version, spanning) in [(3955, false), (2000, true)] {
            let section =
                modern_section(0, &names, |x, y, z| indices[y * 256 + z * 16 + x], spanning);
            let chunk = Chunk::from_nbt(&modern_chunk(version, "full", vec![section])).unwrap();
            for at in [0, 1, 17, 255, 256, 2047, 4095] {
                let (x, y, z) = (at % 16, at / 256, (at / 16) % 16);
                assert_eq!(
                    chunk.sections[0].block(x, y, z),
                    names[usize::from(indices[at])],
                    "palette {palette_len}, version {version}, block {at}"
                );
            }
        }
    }
}

#[test]
fn a_short_data_array_is_an_error_not_a_panic() {
    let mut section = modern_section(0, &[STONE, AIR, WATER], |_, _, _| 1, false);
    if let Tag::Compound(map) = &mut section
        && let Some(Tag::Compound(states)) = map.get_mut("block_states")
    {
        states.insert("data".into(), Tag::LongArray(vec![0; 3]));
    }
    assert!(matches!(
        Chunk::from_nbt(&modern_chunk(3955, "full", vec![section])),
        Err(Error::Format(_))
    ));
}

#[test]
fn columns_show_the_top_block_look_through_plants_and_measure_water() {
    // Section 0: stone to y=9, a water column over (1,0) up to y=12, grass on (2,0).
    let names = [STONE, AIR, WATER, GRASS];
    let low = modern_section(
        0,
        &names,
        |x, y, z| match (x, z) {
            (1, 0) if y <= 12 && y > 9 => 2,
            (2, 0) if y == 10 => 3,
            _ if y <= 9 => 0,
            _ => 1,
        },
        false,
    );
    let empty = modern_section(1, &[AIR], |_, _, _| 0, false);
    let chunk = Chunk::from_nbt(&modern_chunk(3955, "full", vec![low, empty])).unwrap();
    assert_eq!(chunk.sections[0].y, 1, "top first");
    let look_through = |name: &str| name == GRASS;
    let found = columns(&chunk, look_through);
    assert_eq!(found.len(), 256);
    let dry = found[5].unwrap();
    assert_eq!((dry.block, dry.y, dry.water), (STONE, 9, 0));
    let under_water = found[1].unwrap();
    assert_eq!(
        (under_water.block, under_water.y, under_water.water),
        (STONE, 9, 3)
    );
    let grassy = found[2].unwrap();
    assert_eq!(
        (grassy.block, grassy.y),
        (STONE, 9),
        "the plant is looked through"
    );
    let solid = columns(&chunk, |_| false);
    assert_eq!(solid[2].unwrap().block, GRASS);
}

#[test]
fn empty_and_all_water_columns() {
    let air = Chunk::from_nbt(&modern_chunk(
        3955,
        "full",
        vec![modern_section(0, &[AIR], |_, _, _| 0, false)],
    ))
    .unwrap();
    assert!(columns(&air, |_| false).iter().all(Option::is_none));
    let ocean = Chunk::from_nbt(&modern_chunk(
        3955,
        "full",
        vec![modern_section(
            -4,
            &[WATER, AIR],
            |_, y, _| u16::from(y > 5),
            false,
        )],
    ))
    .unwrap();
    let column = columns(&ocean, |_| false)[0].unwrap();
    assert_eq!((column.block, column.y, column.water), (WATER, -64 + 5, 0));
    assert!(
        Chunk::from_nbt(&modern_chunk(3955, "full", vec![]))
            .unwrap()
            .sections
            .is_empty()
    );
}

#[test]
fn the_pre_1_18_layout_and_unsupported_formats() {
    let section = |y: i8, names: &[&str], fill: u16| {
        compound(vec![
            ("Y", Tag::Byte(y)),
            ("Palette", palette(names)),
            (
                "BlockStates",
                Tag::LongArray(pack(&vec![fill; 4096], names.len(), 4, true)),
            ),
        ])
    };
    let level = compound(vec![
        ("Status", Tag::String("full".into())),
        (
            "Sections",
            Tag::List(vec![
                section(0, &[AIR, STONE], 1),
                section(1, &[AIR, WATER], 0),
            ]),
        ),
    ]);
    let root = compound(vec![("DataVersion", Tag::Int(2566)), ("Level", level)]);
    let chunk = Chunk::from_nbt(&root).unwrap();
    assert!(chunk.is_complete());
    assert_eq!(chunk.sections.len(), 2);
    let column = columns(&chunk, |_| false)[17].unwrap();
    assert_eq!((column.block, column.y), (STONE, 15));

    let numeric = compound(vec![
        ("DataVersion", Tag::Int(922)),
        (
            "Level",
            compound(vec![(
                "Sections",
                Tag::List(vec![compound(vec![
                    ("Y", Tag::Byte(0)),
                    ("Blocks", Tag::ByteArray(vec![1; 4096])),
                ])]),
            )]),
        ),
    ]);
    assert_eq!(
        Chunk::from_nbt(&numeric).unwrap_err(),
        Error::Unsupported(UnsupportedVersion(922))
    );
    for (status, complete) in [
        ("minecraft:full", true),
        ("full", true),
        ("minecraft:noise", false),
        ("empty", false),
        ("structure_starts", false),
    ] {
        let chunk = Chunk::from_nbt(&modern_chunk(3955, status, vec![])).unwrap();
        assert_eq!(chunk.is_complete(), complete, "{status}");
    }
}
