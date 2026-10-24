use super::*;
use crate::SettingsStore;

#[test]
fn presets_map_download_paths_and_keep_the_official_fallback() {
    let root = tempfile::tempdir().unwrap();
    let mut store = SettingsStore::open(root.path()).unwrap();
    let rules =
        MirrorPreset::TencentMaven.merge(&MirrorPreset::Mcim.merge(&MirrorPreset::Bmclapi.rules()));
    store.set_mirrors(rules.clone(), true).unwrap();
    let cases = [
        (
            "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
            "https://bmclapi2.bangbang93.com/mc/game/version_manifest_v2.json",
        ),
        (
            "https://piston-data.mojang.com/v1/objects/abc/client.jar",
            "https://bmclapi2.bangbang93.com/v1/objects/abc/client.jar",
        ),
        (
            "http://resources.download.minecraft.net/ab/abcdef",
            "https://bmclapi2.bangbang93.com/assets/ab/abcdef",
        ),
        (
            "https://resources.download.minecraft.net/ab/abcdef",
            "https://bmclapi2.bangbang93.com/assets/ab/abcdef",
        ),
        (
            "https://libraries.minecraft.net/org/example/a.jar",
            "https://bmclapi2.bangbang93.com/libraries/org/example/a.jar",
        ),
        (
            "https://maven.minecraftforge.net/net/minecraftforge/a.jar",
            "https://bmclapi2.bangbang93.com/maven/net/minecraftforge/a.jar",
        ),
        (
            "https://maven.neoforged.net/releases/net/neoforged/a.jar",
            "https://bmclapi2.bangbang93.com/maven/net/neoforged/a.jar",
        ),
        (
            "https://meta.fabricmc.net/v2/versions/loader",
            "https://bmclapi2.bangbang93.com/fabric-meta/v2/versions/loader",
        ),
        (
            "https://authlib-injector.yushi.moe/artifact/latest.json",
            "https://bmclapi2.bangbang93.com/mirrors/authlib-injector/artifact/latest.json",
        ),
        (
            "https://api.modrinth.com/v2/search?query=sodium",
            "https://mod.mcimirror.top/modrinth/v2/search?query=sodium",
        ),
        (
            "https://cdn.modrinth.com/data/abc/versions/def/a.jar",
            "https://mod.mcimirror.top/data/abc/versions/def/a.jar",
        ),
        (
            "https://api.curseforge.com/v1/mods/1",
            "https://mod.mcimirror.top/curseforge/v1/mods/1",
        ),
        (
            "https://edge.forgecdn.net/files/123/456/a.jar",
            "https://mod.mcimirror.top/files/123/456/a.jar",
        ),
        (
            "https://repo.maven.apache.org/maven2/org/example/a.jar",
            "https://mirrors.cloud.tencent.com/nexus/repository/maven-public/org/example/a.jar",
        ),
    ];
    for (original, mirror) in cases {
        let expected = if mirror.starts_with("https://mod.mcimirror.top/") {
            [original, mirror]
        } else {
            [mirror, original]
        };
        assert_eq!(store.source_chain().unwrap().candidates(original), expected);
    }
    let reopened = SettingsStore::open(root.path()).unwrap();
    assert_eq!(reopened.get().mirrors, rules);
    assert!(reopened.get().prefer_mirrors);

    store.set_mirrors(rules, false).unwrap();
    let (original, mirror) = cases[0];
    assert_eq!(
        store.source_chain().unwrap().candidates(original),
        [original, mirror]
    );
    for unrelated in [
        "https://login.microsoftonline.com/token",
        "https://api.modrinth.com.evil.test/v2/search",
        "https://maven.neoforged.net/snapshots/a.jar",
    ] {
        assert_eq!(
            store.source_chain().unwrap().candidates(unrelated),
            [unrelated]
        );
    }
}

#[test]
fn adding_a_preset_is_idempotent_and_preserves_custom_candidate_order() {
    let custom = MirrorRule {
        official_prefix: "https://libraries.minecraft.net/".into(),
        mirror_prefix: "https://custom.test/libraries/".into(),
    };
    let once = MirrorPreset::Bmclapi.merge(std::slice::from_ref(&custom));
    assert_eq!(once[0], custom);
    assert_eq!(MirrorPreset::Bmclapi.merge(&once), once);
    let root = tempfile::tempdir().unwrap();
    let mut store = SettingsStore::open(root.path()).unwrap();
    store.set_mirrors(once, true).unwrap();
    assert_eq!(
        store
            .source_chain()
            .unwrap()
            .candidates("https://libraries.minecraft.net/a.jar"),
        [
            "https://custom.test/libraries/a.jar",
            "https://bmclapi2.bangbang93.com/libraries/a.jar",
            "https://libraries.minecraft.net/a.jar",
        ]
    );
    assert!(
        SettingsStore::open(tempfile::tempdir().unwrap().path())
            .unwrap()
            .get()
            .mirrors
            .is_empty()
    );
}
