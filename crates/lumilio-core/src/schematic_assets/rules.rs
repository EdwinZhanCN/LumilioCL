//! The source rule (D3 as D23 narrows it): the schematic's data version
//! decides which game version's assets are wanted, and vanilla assets always
//! come from the bundle; an installed game is never baked. The instance's
//! mods and resource packs are laid over them only where the studio folder
//! switched them on (plan schematic-studio, T21).

use super::{AssetNotice, AssetPlan, BundleReach, EntityGeometry, InstanceAssets, PlannedSource};

/// The sources for a schematic of `data_version` in `instance`, lowest
/// priority first:
///
/// - the bundle's pack for the schematic's version (between versions the
///   next newer; without a data version, the instance's own version);
/// - out of reach (no network and nothing kept, or download turned off):
///   no sources, and a notice saying which;
/// - then the instance's overlays that are switched on, in their order.
///
/// Entity geometry (D4) is the bundle's `entities.dat.z` for that version,
/// when the bundle has one.
#[must_use]
pub fn resolve(
    instance: &InstanceAssets,
    data_version: Option<u32>,
    bundle: BundleReach<'_>,
) -> AssetPlan {
    let index = match bundle {
        BundleReach::Index(index) => index,
        BundleReach::Offline => return without(AssetNotice::Offline),
        BundleReach::Unavailable => return without(AssetNotice::Unavailable),
        BundleReach::Disabled => return without(AssetNotice::DownloadDisabled),
    };
    let version = index
        .version_for(data_version, &instance.game_version)
        .id
        .clone();
    let entities = if index.covers_entities(&version) {
        EntityGeometry::Bundle {
            version: version.clone(),
        }
    } else {
        EntityGeometry::None
    };
    let mut sources = vec![PlannedSource::Bundle { version }];
    sources.extend(
        instance
            .overlays
            .iter()
            .filter(|overlay| instance.switched_on.contains(overlay))
            .cloned()
            .map(PlannedSource::Overlay),
    );
    AssetPlan {
        sources,
        notice: None,
        entities,
        provided: Vec::new(),
    }
}

fn without(notice: AssetNotice) -> AssetPlan {
    AssetPlan {
        notice: Some(notice),
        ..AssetPlan::default()
    }
}
