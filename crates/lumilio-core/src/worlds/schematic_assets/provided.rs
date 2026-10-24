//! What each source of a preview provides, for the studio's 资源来源
//! (plan schematic-studio, T17, T21): the bundle's base, then every mod and
//! resource pack on offer that holds assets, switched on or not, with the
//! blocks it adds or redraws and the models its custom loader keeps from
//! baking.

use std::path::{Path, PathBuf};

use lumilio_block_assets::BakedPack;

use super::{AssetPlan, InstanceAssets, PlannedSource, Scan};

/// One source on offer and what it provides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProvidedSource {
    pub role: SourceRole,
    /// Blockstate files: the blocks it adds or redraws.
    pub blocks: u32,
    /// Asset files of any kind.
    pub assets: u32,
    /// Models a custom loader draws (Forge OBJ, composite …), which the baker
    /// cannot: they show as the missing-model cube.
    pub custom_models: u32,
    /// Whether the preview draws with it. The base always; an overlay only
    /// once its studio folder switched it on (T21, D23).
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceRole {
    /// The bundle's vanilla assets of `version`.
    Base {
        version: String,
    },
    Mod(PathBuf),
    /// A resource pack the game enables.
    ResourcePack(PathBuf),
    /// A pack added in the studio; `missing` once its file is gone.
    Added {
        path: PathBuf,
        missing: bool,
    },
}

impl SourceRole {
    /// The file a mod or pack is read from; `None` for the base.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Base { .. } => None,
            Self::Mod(path) | Self::ResourcePack(path) | Self::Added { path, .. } => Some(path),
        }
    }
}

/// The sources of `pack`, loaded from `plan` for `instance`, in priority
/// order (lowest first): the base, then every overlay `instance` offers,
/// counted by `scan` whether it is switched on or not. Overlays without
/// assets (library mods, jars that are no zip) are left out; packs added in
/// the studio always show, so they can be taken off again.
#[must_use]
pub fn provided(
    plan: &AssetPlan,
    pack: &BakedPack,
    instance: &InstanceAssets,
    scan: impl Fn(&Path) -> Scan,
) -> Vec<ProvidedSource> {
    let Some(PlannedSource::Bundle { version }) = plan.base() else {
        return Vec::new();
    };
    let stamps = &pack.meta.sources;
    let base_blocks = match stamps.first() {
        Some(stamp) if stamp.blocks > 0 => stamp.blocks,
        // A bundle baked before stamps counted.
        _ => pack
            .assets
            .blockstates
            .keys()
            .filter(|id| id.starts_with("minecraft:"))
            .count() as u32,
    };
    let mut list = vec![ProvidedSource {
        role: SourceRole::Base {
            version: version.clone(),
        },
        blocks: base_blocks,
        assets: stamps.first().map_or(0, |stamp| stamp.assets),
        custom_models: 0,
        enabled: true,
    }];
    let on = plan.overlays();
    for path in &instance.overlays {
        let counts = scan(path);
        let (blocks, assets, custom_models) = if counts.unreadable {
            (0, 0, 0)
        } else {
            (counts.blocks, counts.assets, counts.custom_models)
        };
        let is_added = instance.added.contains(path);
        if assets == 0 && !is_added {
            continue;
        }
        list.push(ProvidedSource {
            role: role_of(path, is_added),
            blocks,
            assets,
            custom_models,
            enabled: on.contains(&path.as_path()),
        });
    }
    for path in &instance.added {
        if !instance.overlays.contains(path) {
            list.push(ProvidedSource {
                role: SourceRole::Added {
                    path: path.clone(),
                    missing: true,
                },
                blocks: 0,
                assets: 0,
                custom_models: 0,
                enabled: false,
            });
        }
    }
    list
}

fn role_of(path: &Path, added: bool) -> SourceRole {
    if added {
        SourceRole::Added {
            path: path.to_owned(),
            missing: false,
        }
    } else if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"))
    {
        SourceRole::Mod(path.to_owned())
    } else {
        SourceRole::ResourcePack(path.to_owned())
    }
}
