use super::*;
use crate::discover::{PackFileKind, VersionFile};

const MANIFEST_JSON: &str = r#"{
  "minecraft": {"version": "1.20.1", "modLoaders": [
    {"id": "fabric-0.14.0", "primary": false},
    {"id": "neoforge-1.20.1-47.1.106", "primary": true}]},
  "manifestType": "minecraftModpack", "manifestVersion": 1,
  "name": " Better Pack ", "version": "4.2", "author": "Someone",
  "files": [
    {"projectID": 1, "fileID": 10, "required": true},
    {"projectID": 2, "fileID": 20, "required": false},
    {"projectID": 3, "fileID": 30},
    {"projectID": 4, "fileID": 40},
    {"projectID": 5, "fileID": 50},
    {"projectID": 6, "fileID": 60},
    {"projectID": 7, "fileID": 70},
    {"projectID": 1, "fileID": 10}],
  "overrides": "overrides"
}"#;

fn resolved(
    project: &str,
    version: &str,
    kind: PackFileKind,
    url: &str,
    sha1: &str,
) -> ResolvedFile {
    ResolvedFile {
        project_id: project.to_owned(),
        version_id: version.to_owned(),
        title: format!("Project {project}"),
        kind,
        file: VersionFile {
            url: url.to_owned(),
            filename: format!("p{project}.jar"),
            primary: true,
            size: 7,
            sha1: Some(sha1.to_owned()),
            download_page: Some(format!(
                "https://www.curseforge.com/minecraft/mc-mods/p{project}/files/{version}"
            )),
        },
    }
}

#[test]
fn the_manifest_names_game_loader_overrides_and_files() {
    let manifest = parse_manifest(MANIFEST_JSON).unwrap();
    assert_eq!(manifest.name, "Better Pack");
    assert_eq!(manifest.minecraft, "1.20.1");
    assert_eq!(manifest.loader, Loader::NeoForge, "the primary loader");
    assert_eq!(
        manifest.loader_version.as_deref(),
        Some("47.1.106"),
        "without the game version it repeats"
    );
    assert_eq!(manifest.overrides, "overrides");
    assert_eq!(manifest.files.len(), 8);
    assert!(!manifest.files[1].required);
    assert!(
        manifest.files[2].required,
        "required unless it says otherwise"
    );
}

#[test]
fn bad_manifests_are_refused_with_a_reason() {
    for (json, why) in [
        (
            r#"{"manifestType":"other","minecraft":{"version":"1.20.1"},"name":"x"}"#,
            "type",
        ),
        (r#"{"minecraft":{"modLoaders":[]},"name":"x"}"#, "version"),
        (
            r#"{"minecraft":{"version":"1.20.1","modLoaders":[{"id":"rift-1"}]},"name":"x"}"#,
            "loader",
        ),
        (
            r#"{"minecraft":{"version":"1.20.1"},"name":"x","overrides":"../up"}"#,
            "overrides",
        ),
        (r#"{"minecraft":{"version":"1.20.1"},"name":" "}"#, "name"),
        (
            r#"{"minecraft":{"version":"1.20.1"},"name":"x","files":[{"projectID":1}]}"#,
            "file",
        ),
    ] {
        assert!(parse_manifest(json).is_err(), "{why}");
    }
    let vanilla = parse_manifest(r#"{"minecraft":{"version":"1.20.1"},"name":"x"}"#).unwrap();
    assert_eq!(
        (vanilla.loader, vanilla.loader_version),
        (Loader::Vanilla, None)
    );
}

#[test]
fn files_go_to_their_folders_and_what_cannot_be_downloaded_is_listed() {
    let manifest = parse_manifest(MANIFEST_JSON).unwrap();
    let found = vec![
        resolved(
            "1",
            "10",
            PackFileKind::Mod,
            "https://edge.forgecdn.net/files/1/p1.jar",
            "a1",
        ),
        resolved(
            "2",
            "20",
            PackFileKind::ResourcePack,
            "https://edge.forgecdn.net/files/2/p2.jar",
            "a2",
        ),
        resolved(
            "3",
            "30",
            PackFileKind::Shader,
            "https://edge.forgecdn.net/files/3/p3.jar",
            "a3",
        ),
        resolved("4", "40", PackFileKind::DataPack, "", "a4"),
        resolved("5", "50", PackFileKind::Mod, "", "a5"),
        resolved(
            "6",
            "60",
            PackFileKind::Other,
            "https://edge.forgecdn.net/files/6/p6.jar",
            "a6",
        ),
    ];
    let elsewhere = BTreeMap::from([(
        "a4".to_owned(),
        "https://cdn.modrinth.com/data/x/p4.jar".to_owned(),
    )]);
    let index = index(&manifest, &found, &elsewhere).unwrap();
    let placed: Vec<(&str, &str)> = index
        .files
        .iter()
        .map(|file| (file.path.as_str(), file.downloads[0].as_str()))
        .collect();
    assert_eq!(
        placed,
        vec![
            ("mods/p1.jar", "https://edge.forgecdn.net/files/1/p1.jar"),
            (
                "resourcepacks/p2.jar.disabled",
                "https://edge.forgecdn.net/files/2/p2.jar"
            ),
            (
                "shaderpacks/p3.jar",
                "https://edge.forgecdn.net/files/3/p3.jar"
            ),
            ("datapacks/p4.jar", "https://cdn.modrinth.com/data/x/p4.jar"),
        ],
        "a file listed twice is installed once"
    );
    assert_eq!(index.files[0].sha1.as_deref(), Some("a1"));
    let manual: Vec<(&str, &str, Option<&str>)> = index
        .manual
        .iter()
        .map(|file| {
            (
                file.file_name.as_str(),
                file.path.as_str(),
                file.page.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        manual,
        vec![
            (
                "p5.jar",
                "mods/p5.jar",
                Some("https://www.curseforge.com/minecraft/mc-mods/p5/files/50")
            ),
            (
                "p6.jar",
                "",
                Some("https://www.curseforge.com/minecraft/mc-mods/p6/files/60")
            ),
            ("7/70", "", Some("https://www.curseforge.com/projects/7")),
        ]
    );
    assert_eq!(index.manual[0].title, "Project 5");
    assert_eq!(index.overrides, vec!["overrides/"]);
    assert_eq!(
        (index.loader, index.minecraft.as_str()),
        (Loader::NeoForge, "1.20.1")
    );
}

#[test]
fn an_unsafe_file_name_refuses_the_pack() {
    let manifest = parse_manifest(MANIFEST_JSON).unwrap();
    let mut evil = resolved(
        "1",
        "10",
        PackFileKind::Mod,
        "https://edge.forgecdn.net/x",
        "a1",
    );
    evil.file.filename = "../../evil.jar".to_owned();
    assert!(matches!(
        index(&manifest, &[evil], &BTreeMap::new()),
        Err(ModpackError::UnsafePath(_))
    ));
}

#[test]
fn only_curseforge_and_modrinth_cdns_are_trusted() {
    assert!(is_trusted_source(
        "https://edge.forgecdn.net/files/1/2/a.jar"
    ));
    assert!(is_trusted_source(
        "https://mediafilez.forgecdn.net/files/1/2/a.jar"
    ));
    assert!(is_trusted_source("https://cdn.modrinth.com/data/a.jar"));
    assert!(!is_trusted_source(
        "http://edge.forgecdn.net/files/1/2/a.jar"
    ));
    assert!(!is_trusted_source("https://example.com/a.jar"));
}
