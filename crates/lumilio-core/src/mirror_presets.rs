//! Optional download mirrors; adding a preset never changes source preference.

use crate::MirrorRule;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirrorPreset {
    Bmclapi,
    Mcim,
    TencentMaven,
}

// URL mappings adapted from HMCL:
// HMCLCore/src/main/java/org/jackhuang/hmcl/download/BMCLAPIDownloadProvider.java
// Copyright (C) 2020 huangyuhui and contributors, GPL-3.0-or-later.
// Minecraft asset mappings also follow that provider's getAssetObjectCandidates.
// Use path boundaries because PrefixMirror deliberately performs a literal rewrite.
const BMCLAPI: &[(&str, &str)] = &[
    (
        "https://launchermeta.mojang.com/",
        "https://bmclapi2.bangbang93.com/",
    ),
    (
        "https://piston-meta.mojang.com/",
        "https://bmclapi2.bangbang93.com/",
    ),
    (
        "https://piston-data.mojang.com/",
        "https://bmclapi2.bangbang93.com/",
    ),
    (
        "https://launcher.mojang.com/",
        "https://bmclapi2.bangbang93.com/",
    ),
    (
        "https://resources.download.minecraft.net/",
        "https://bmclapi2.bangbang93.com/assets/",
    ),
    (
        "http://resources.download.minecraft.net/",
        "https://bmclapi2.bangbang93.com/assets/",
    ),
    (
        "https://libraries.minecraft.net/",
        "https://bmclapi2.bangbang93.com/libraries/",
    ),
    (
        "http://files.minecraftforge.net/maven/",
        "https://bmclapi2.bangbang93.com/maven/",
    ),
    (
        "https://files.minecraftforge.net/maven/",
        "https://bmclapi2.bangbang93.com/maven/",
    ),
    (
        "https://maven.minecraftforge.net/",
        "https://bmclapi2.bangbang93.com/maven/",
    ),
    (
        "https://maven.neoforged.net/releases/",
        "https://bmclapi2.bangbang93.com/maven/",
    ),
    (
        "https://meta.fabricmc.net/",
        "https://bmclapi2.bangbang93.com/fabric-meta/",
    ),
    (
        "https://maven.fabricmc.net/",
        "https://bmclapi2.bangbang93.com/maven/",
    ),
    (
        "https://authlib-injector.yushi.moe/",
        "https://bmclapi2.bangbang93.com/mirrors/authlib-injector/",
    ),
];

const MCIM: &[(&str, &str)] = &[
    (
        "https://api.modrinth.com/",
        "https://mod.mcimirror.top/modrinth/",
    ),
    ("https://cdn.modrinth.com/", "https://mod.mcimirror.top/"),
    (
        "https://api.curseforge.com/",
        "https://mod.mcimirror.top/curseforge/",
    ),
    ("https://edge.forgecdn.net/", "https://mod.mcimirror.top/"),
];

const TENCENT_MAVEN: &[(&str, &str)] = &[
    (
        "https://repo1.maven.org/maven2/",
        "https://mirrors.cloud.tencent.com/nexus/repository/maven-public/",
    ),
    (
        "https://repo.maven.apache.org/maven2/",
        "https://mirrors.cloud.tencent.com/nexus/repository/maven-public/",
    ),
];

impl MirrorPreset {
    #[must_use]
    pub fn rules(self) -> Vec<MirrorRule> {
        let mappings = match self {
            Self::Bmclapi => BMCLAPI,
            Self::Mcim => MCIM,
            Self::TencentMaven => TENCENT_MAVEN,
        };
        mappings
            .iter()
            .map(|(official, mirror)| MirrorRule {
                official_prefix: (*official).to_owned(),
                mirror_prefix: (*mirror).to_owned(),
            })
            .collect()
    }

    /// Append missing rules, retaining custom rules and their candidate order.
    #[must_use]
    pub fn merge(self, existing: &[MirrorRule]) -> Vec<MirrorRule> {
        let mut merged = existing.to_vec();
        for rule in self.rules() {
            if !merged.contains(&rule) {
                merged.push(rule);
            }
        }
        merged
    }
}

#[cfg(test)]
mod tests;
