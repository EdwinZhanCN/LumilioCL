//! Converting a schematic between Minecraft data versions (schematic-studio T14).
//!
//! Forward conversion (to a newer version) is lossless. Backward conversion
//! approximates what the older version cannot hold and reports each loss, so
//! the host can show them before it writes anything.
//!
//! Nucleation reports losses only where it inverts a converter step (the
//! flattening, renamed blocks). A block the older version never had passes
//! through unchanged and unreported, so the host may also pass the block
//! states the target version knows ([`Catalogs`], read from its baked
//! blockstates): every converted block state is checked against them.

use std::collections::BTreeMap;

pub use lumilio_block_assets::BlockCatalog;
use nucleation::dataconverter::{
    CANONICAL_DATA_VERSION, LossReport, Severity as NucleationSeverity,
};

use crate::stats::stats;
use crate::{Error, UniversalSchematic};

/// [`Loss::kind`] of a block the target version does not have. The block is
/// written as it is; the older game reads it as air.
pub const MISSING_BLOCK: &str = "missing_block";
/// [`Loss::kind`] of a property or value the target version does not know.
/// The state is written as it is; the older game drops the property and
/// uses the block's default for it.
pub const UNKNOWN_PROPERTY: &str = "unknown_property";

/// Which way a conversion goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Already at the target version.
    Same,
    /// To a newer version: lossless.
    Forward,
    /// To an older version: may lose data.
    Backward,
}

/// How bad one loss is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// A usually correct substitute was used.
    Approximated,
    /// The original value cannot be recovered.
    Lost,
}

/// One thing a backward conversion could not keep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loss {
    pub severity: Severity,
    /// A stable category tag such as `flattening_unknown_block`,
    /// [`MISSING_BLOCK`] or [`UNKNOWN_PROPERTY`].
    pub kind: &'static str,
    /// The data version whose converter was inverted, or the target's for a
    /// catalog check.
    pub version: i32,
    /// Where the datum was, e.g. `block_entity minecraft:chest @ (1,2,3)`;
    /// for a catalog check the block (`minecraft:cherry_log`) or the state
    /// (`minecraft:lantern[hanging=true]`).
    pub path: String,
    pub detail: String,
    /// How many blocks it concerns: 1 for a single datum.
    pub count: u64,
}

/// The block states a backward conversion is checked against.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalogs {
    /// What the target version knows.
    pub target: BlockCatalog,
    /// What the source version's blockstates mention, when known. A property
    /// the target does not mention is reported only when the source does:
    /// properties no model depends on (`waterlogged`) are mentioned by neither.
    pub source: Option<BlockCatalog>,
}

/// The result of a conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversion {
    pub from: i32,
    pub to: i32,
    pub direction: Direction,
    /// Worst first. Always empty for forward conversion.
    pub losses: Vec<Loss>,
}

impl Conversion {
    pub fn is_lossless(&self) -> bool {
        self.losses.is_empty()
    }
}

/// The data version the schematic's content is at now, as its file says.
/// `None` when the file names none (a `.litematic` without
/// `MinecraftDataVersion`): the version is unknown, and the studio does not
/// guess one or convert on its behalf.
pub fn known_data_version(schematic: &UniversalSchematic) -> Option<i32> {
    schematic
        .metadata
        .source_data_version
        .or(schematic.metadata.mc_version)
}

/// The data version a conversion starts from: [`known_data_version`], or
/// Nucleation's canonical version when the file names none. Callers that
/// show or act on a version check [`known_data_version`] first.
pub fn data_version(schematic: &UniversalSchematic) -> i32 {
    known_data_version(schematic).unwrap_or(CANONICAL_DATA_VERSION)
}

pub fn direction(from: i32, to: i32) -> Direction {
    match to.cmp(&from) {
        std::cmp::Ordering::Equal => Direction::Same,
        std::cmp::Ordering::Greater => Direction::Forward,
        std::cmp::Ordering::Less => Direction::Backward,
    }
}

/// Converts the schematic in place to `target` and reports what was lost,
/// checking the result against `catalogs` when given.
pub fn convert(
    schematic: &mut UniversalSchematic,
    target: i32,
    catalogs: Option<&Catalogs>,
) -> Conversion {
    let from = data_version(schematic);
    let report = schematic.convert_to_data_version(target);
    conversion(from, target, &report, schematic, catalogs)
}

/// Writes `.litematic` bytes for `target` from a converted copy; the input is
/// left as it was. The host shows the losses and writes only on confirmation.
pub fn to_bytes_for_data_version(
    schematic: &UniversalSchematic,
    target: i32,
    catalogs: Option<&Catalogs>,
) -> Result<(Vec<u8>, Conversion), Error> {
    let mut converted = schematic.clone();
    let conversion = convert(&mut converted, target, catalogs);
    let bytes = nucleation::litematic::to_litematic(&converted)
        .map_err(|error| Error(error.to_string()))?;
    Ok((bytes, conversion))
}

fn conversion(
    from: i32,
    to: i32,
    report: &LossReport,
    converted: &UniversalSchematic,
    catalogs: Option<&Catalogs>,
) -> Conversion {
    let direction = direction(from, to);
    let mut losses: Vec<Loss> = report
        .entries
        .iter()
        .map(|entry| Loss {
            severity: match entry.severity {
                NucleationSeverity::Loss => Severity::Lost,
                NucleationSeverity::Approximated => Severity::Approximated,
            },
            kind: entry.kind.as_str(),
            version: entry.version,
            path: entry.path.clone(),
            detail: entry.detail.clone(),
            count: 1,
        })
        .collect();
    if direction == Direction::Backward
        && let Some(catalogs) = catalogs
    {
        losses.extend(unknown_states(converted, to, catalogs));
    }
    losses.sort_by_key(|loss| std::cmp::Reverse(loss.severity));
    Conversion {
        from,
        to,
        direction,
        losses,
    }
}

/// Each converted block the target does not have, and each state whose
/// property or value it does not know, with how many blocks hold it.
fn unknown_states(converted: &UniversalSchematic, version: i32, catalogs: &Catalogs) -> Vec<Loss> {
    let mut losses = Vec::new();
    for block in stats(converted).blocks {
        let id = block.id.as_str();
        if !catalogs.target.has_block(id) {
            losses.push(Loss {
                severity: Severity::Lost,
                kind: MISSING_BLOCK,
                version,
                path: block.id.clone(),
                detail: format!("{id} does not exist in data version {version}"),
                count: block.count,
            });
            continue;
        }
        // States that differ only in known properties share one entry.
        let mut unknown_states: BTreeMap<Vec<String>, u64> = BTreeMap::new();
        for state in &block.states {
            let unknown: Vec<&(String, String)> = state
                .properties
                .iter()
                .filter(|(name, value)| match catalogs.target.property(id, name) {
                    Some(values) => !values.allows(value),
                    None => catalogs
                        .source
                        .as_ref()
                        .is_some_and(|source| source.property(id, name).is_some()),
                })
                .collect();
            if unknown.is_empty() {
                continue;
            }
            let listed: Vec<String> = unknown
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect();
            *unknown_states.entry(listed).or_default() += state.count;
        }
        for (listed, count) in unknown_states {
            losses.push(Loss {
                severity: Severity::Approximated,
                kind: UNKNOWN_PROPERTY,
                version,
                path: format!("{id}[{}]", listed.join(",")),
                detail: format!(
                    "data version {version} does not know {}; the default is used",
                    listed.join(", ")
                ),
                count,
            });
        }
    }
    losses
}
