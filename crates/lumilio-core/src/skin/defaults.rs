//! UUID selection follows HMCL `HMCL/.../game/TexturesLoader.java`,
//! Copyright (C) 2021 huangyuhui and contributors, GPL-3.0-or-later (ADR 0011).
//! Artwork is read only from an installed client; none is distributed here.

use super::{AccountLook, SkinModel, skin_pixels};
use crate::account::ProfileId;
use std::{fs::File, io::Read, path::Path};

pub(super) fn uuid_hash(id: ProfileId) -> i32 {
    let hex = id.compact();
    (0..4).fold(0_u32, |hash, ix| {
        hash ^ u32::from_str_radix(&hex[ix * 8..ix * 8 + 8], 16).unwrap_or(0)
    }) as i32
}

pub(crate) fn from_client(jar: &Path, id: ProfileId) -> Option<AccountLook> {
    let mut archive = zip::ZipArchive::new(File::open(jar).ok()?).ok()?;
    let names = [
        "alex", "ari", "efe", "kai", "makena", "noor", "steve", "sunny", "zuri",
    ];
    let hash = uuid_hash(id);
    let ix = hash.rem_euclid(18) as usize;
    let model = if ix < 9 {
        SkinModel::Slim
    } else {
        SkinModel::Wide
    };
    let shape = if model == SkinModel::Slim {
        "slim"
    } else {
        "wide"
    };
    let modern = format!(
        "assets/minecraft/textures/entity/player/{shape}/{}.png",
        names[ix % 9]
    );
    // Earlier clients only have Steve/Alex; before 1.8 only Steve exists.
    let legacy = if hash & 1 == 1 { "alex" } else { "steve" };
    let paths = [
        modern,
        format!("assets/minecraft/textures/entity/{legacy}.png"),
        "assets/minecraft/textures/entity/steve.png".into(),
    ];
    for (ix, path) in paths.iter().enumerate() {
        let Ok(mut entry) = archive.by_name(path) else {
            continue;
        };
        if entry.size() > 2 * 1024 * 1024 {
            continue;
        }
        let mut bytes = Vec::new();
        if entry
            .by_ref()
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .is_err()
        {
            continue;
        }
        let Ok(skin) = skin_pixels(&bytes) else {
            continue;
        };
        return Some(AccountLook {
            skin: Some(skin),
            cape: None,
            model: if ix == 0 {
                model
            } else if ix == 1 && legacy == "alex" {
                SkinModel::Slim
            } else {
                SkinModel::Wide
            },
        });
    }
    None
}

#[cfg(test)]
mod tests;
