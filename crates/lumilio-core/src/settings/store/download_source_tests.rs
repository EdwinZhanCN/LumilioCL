use super::*;
use crate::MirrorPreset;

#[test]
fn legacy_download_source_preferences_survive_migration() {
    for (legacy, expected) in [
        (false, DownloadSourcePreference::OfficialFirst),
        (true, DownloadSourcePreference::MirrorFirst),
    ] {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE),
            format!(r#"{{"schema":1,"prefer_mirrors":{legacy}}}"#),
        )
        .unwrap();
        let mut store = SettingsStore::open(dir.path()).unwrap();
        assert!(!store.recovered_from_damage());
        assert_eq!(store.get().download_source_preference(), expected);
        store
            .set_download_source(DownloadSourcePreference::OfficialOnly)
            .unwrap();
        store.set_mirror_rules(MirrorPreset::Mcim.rules()).unwrap();
        let reopened = SettingsStore::open(dir.path()).unwrap();
        assert_eq!(
            reopened.get().download_source_preference(),
            DownloadSourcePreference::OfficialOnly
        );
        assert_eq!(reopened.get().mirrors, MirrorPreset::Mcim.rules());
    }
}

#[test]
fn download_source_modes_keep_mcim_after_official_and_preserve_rules() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SettingsStore::open(dir.path()).unwrap();
    let original = "https://api.modrinth.com/v2/search";
    let custom = "https://custom.test/v2/search";
    let mcim = "https://mod.mcimirror.top/modrinth/v2/search";
    let rules = MirrorPreset::Mcim.merge(&[MirrorRule {
        official_prefix: "https://api.modrinth.com/".into(),
        mirror_prefix: "https://custom.test/".into(),
    }]);
    store.set_mirror_rules(rules.clone()).unwrap();
    for (mode, expected) in [
        (
            DownloadSourcePreference::OfficialFirst,
            vec![original, custom, mcim],
        ),
        (
            DownloadSourcePreference::MirrorFirst,
            vec![custom, original, mcim],
        ),
        (DownloadSourcePreference::OfficialOnly, vec![original]),
    ] {
        store.set_download_source(mode).unwrap();
        let reopened = SettingsStore::open(dir.path()).unwrap();
        assert_eq!(
            reopened.source_chain().unwrap().candidates(original),
            expected
        );
        assert_eq!(reopened.get().mirrors, rules);
        assert_eq!(reopened.get().download_source_preference(), mode);
    }
}

#[test]
fn failed_download_source_save_does_not_publish_the_new_preference() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SettingsStore::open(dir.path()).unwrap();
    store
        .set_download_source(DownloadSourcePreference::OfficialOnly)
        .unwrap();
    std::fs::create_dir(dir.path().join("settings.json.tmp")).unwrap();
    assert!(
        store
            .set_download_source(DownloadSourcePreference::MirrorFirst)
            .is_err()
    );
    assert!(
        store
            .set_mirror_rules(MirrorPreset::Bmclapi.rules())
            .is_err()
    );
    assert_eq!(
        store.get().download_source_preference(),
        DownloadSourcePreference::OfficialOnly
    );
    assert!(store.get().mirrors.is_empty());
}
