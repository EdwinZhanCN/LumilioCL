use super::{cells, state};
use crate::convert::{
    BlockCatalog, Catalogs, Direction, MISSING_BLOCK, Severity, UNKNOWN_PROPERTY, convert,
    data_version, to_bytes_for_data_version,
};
use crate::{UniversalSchematic, load_litematic};
use nucleation::dataconverter::CANONICAL_DATA_VERSION;

/// Minecraft 1.16.5.
const OLD: i32 = 2586;
/// Minecraft 1.12.2, before the flattening.
const PRE_FLATTENING: i32 = 1343;

fn old_build() -> UniversalSchematic {
    let mut schematic = UniversalSchematic::new("old".to_owned());
    schematic.metadata.mc_version = Some(OLD);
    schematic.metadata.source_data_version = Some(OLD);
    schematic.set_block(0, 0, 0, &state("minecraft:stone"));
    schematic.set_block(
        1,
        0,
        0,
        &state("minecraft:oak_stairs[facing=east,half=top,shape=straight,waterlogged=false]"),
    );
    schematic.set_block(2, 0, 0, &state("minecraft:grass_path"));
    schematic
}

#[test]
fn forward_then_backward_returns_the_original_blocks() {
    let original = old_build();
    let mut schematic = original.clone();

    let forward = convert(&mut schematic, CANONICAL_DATA_VERSION, None);
    assert_eq!(forward.direction, Direction::Forward);
    assert!(forward.is_lossless());
    assert_eq!(data_version(&schematic), CANONICAL_DATA_VERSION);
    assert_ne!(cells(&schematic), cells(&original), "content was converted");

    let backward = convert(&mut schematic, OLD, None);
    assert_eq!(backward.direction, Direction::Backward);
    assert_eq!(cells(&schematic), cells(&original));
}

#[test]
fn backward_conversion_reports_losses() {
    let mut schematic = UniversalSchematic::new("new".to_owned());
    schematic.metadata.mc_version = Some(CANONICAL_DATA_VERSION);
    schematic.set_block(0, 0, 0, &state("minecraft:cherry_log[axis=y]"));

    let conversion = convert(&mut schematic, PRE_FLATTENING, None);
    assert_eq!(conversion.direction, Direction::Backward);
    assert!(!conversion.is_lossless(), "{conversion:?}");
    assert!(conversion.losses.iter().all(|loss| !loss.kind.is_empty()));
    assert!(
        conversion
            .losses
            .windows(2)
            .all(|pair| pair[0].severity >= pair[1].severity),
        "worst first"
    );
}

#[test]
fn bytes_for_a_data_version_leave_the_input_alone() {
    let schematic = old_build();
    let (bytes, conversion) =
        to_bytes_for_data_version(&schematic, CANONICAL_DATA_VERSION, None).unwrap();
    assert_eq!(conversion.direction, Direction::Forward);
    assert!(conversion.is_lossless());
    assert_eq!(data_version(&schematic), OLD);

    let written = load_litematic(&bytes).unwrap();
    assert_eq!(data_version(&written), CANONICAL_DATA_VERSION);
    assert_eq!(cells(&written).len(), cells(&schematic).len());
}

/// Minecraft 1.13.2, the oldest version the block-asset bundle holds.
const BUNDLE_OLDEST: i32 = 1631;

/// What a 1.13.2 bundle slice says, cut down: logs without cherry, a lantern
/// that cannot hang, leaves whose blockstate names no property.
fn catalogs() -> Catalogs {
    let mut target = BlockCatalog::default();
    for axis in ["x", "y", "z"] {
        target.add_variant("minecraft:oak_log", &format!("axis={axis}"));
    }
    target.add_variant("minecraft:lantern", "");
    target.add_variant("minecraft:oak_leaves", "");
    let mut source = target.clone();
    for axis in ["x", "y", "z"] {
        source.add_variant("minecraft:cherry_log", &format!("axis={axis}"));
    }
    source.add_variant("minecraft:lantern", "hanging=false");
    source.add_variant("minecraft:lantern", "hanging=true");
    Catalogs {
        target,
        source: Some(source),
    }
}

fn newest_with(blocks: &[&str]) -> UniversalSchematic {
    let mut schematic = UniversalSchematic::new("new".to_owned());
    schematic.metadata.mc_version = Some(CANONICAL_DATA_VERSION);
    for (x, block) in blocks.iter().enumerate() {
        schematic.set_block(i32::try_from(x).unwrap(), 0, 0, &state(block));
    }
    schematic
}

#[test]
fn a_block_the_target_does_not_have_is_a_loss() {
    let schematic = newest_with(&[
        "minecraft:cherry_log[axis=y]",
        "minecraft:cherry_log[axis=x]",
    ]);
    // The gap this check closes: Nucleation alone reports nothing here.
    let (_, unchecked) = to_bytes_for_data_version(&schematic, BUNDLE_OLDEST, None).unwrap();
    assert!(unchecked.is_lossless(), "{unchecked:?}");

    let (bytes, conversion) =
        to_bytes_for_data_version(&schematic, BUNDLE_OLDEST, Some(&catalogs())).unwrap();
    let missing: Vec<_> = conversion
        .losses
        .iter()
        .filter(|loss| loss.kind == MISSING_BLOCK)
        .collect();
    assert_eq!(missing.len(), 1, "{conversion:?}");
    assert_eq!(missing[0].severity, Severity::Lost);
    assert_eq!(missing[0].path, "minecraft:cherry_log");
    assert_eq!(missing[0].count, 2);
    assert_eq!(missing[0].version, BUNDLE_OLDEST);
    // Like Nucleation, the converter writes the block as it is.
    let written = load_litematic(&bytes).unwrap();
    assert_eq!(data_version(&written), BUNDLE_OLDEST);
    assert!(
        cells(&written)
            .iter()
            .all(|(_, state)| state.starts_with("minecraft:cherry_log"))
    );
}

#[test]
fn a_block_both_versions_know_alike_is_no_loss() {
    let mut schematic = newest_with(&[
        "minecraft:oak_log[axis=z]",
        // No blockstate names these: they are not reported either way.
        "minecraft:oak_leaves[distance=7,persistent=true,waterlogged=false]",
    ]);
    let conversion = convert(&mut schematic, BUNDLE_OLDEST, Some(&catalogs()));
    assert_eq!(conversion.direction, Direction::Backward);
    assert!(conversion.is_lossless(), "{conversion:?}");
}

#[test]
fn a_property_the_target_does_not_know_is_an_approximation() {
    let mut schematic = newest_with(&[
        "minecraft:lantern[hanging=true,waterlogged=false]",
        "minecraft:lantern[hanging=true,waterlogged=true]",
        // A value the target's variant keys do not list.
        "minecraft:oak_log[axis=w]",
    ]);
    let conversion = convert(&mut schematic, BUNDLE_OLDEST, Some(&catalogs()));
    let unknown: Vec<(&str, u64, Severity)> = conversion
        .losses
        .iter()
        .filter(|loss| loss.kind == UNKNOWN_PROPERTY)
        .map(|loss| (loss.path.as_str(), loss.count, loss.severity))
        .collect();
    assert_eq!(
        unknown,
        [
            ("minecraft:lantern[hanging=true]", 2, Severity::Approximated),
            ("minecraft:oak_log[axis=w]", 1, Severity::Approximated),
        ]
    );
}

#[test]
fn a_forward_conversion_is_not_checked() {
    let mut schematic = old_build();
    let empty = Catalogs::default();
    let conversion = convert(&mut schematic, CANONICAL_DATA_VERSION, Some(&empty));
    assert!(conversion.is_lossless());
}
