//! Two content sources at once: Modrinth first, CurseForge second.

use std::collections::BTreeMap;

use crate::activity::CancellationToken;
use crate::content::discover::{DEFAULT_SOURCE, ProjectKind, ProjectOrigin};
use crate::content::{AddedFile, ContentLookup};
use crate::error::ServiceError;
use crate::instances::Loader;
use crate::testing::*;

const CF: &str = lumilio_plugin_curseforge::ID;
const CF_API: &str = "https://api.curseforge.com";

fn murmur(bytes: &[u8]) -> u32 {
    crate::content::fingerprint::of_bytes(bytes)
}

/// A CurseForge file, downloadable from `url` (empty: only on the site).
fn cf_file(id: u64, project: u64, name: &str, body: &[u8], url: &str) -> String {
    let url = if url.is_empty() {
        "null".to_owned()
    } else {
        format!("\"{url}\"")
    };
    format!(
        r#"{{"id":{id},"modId":{project},"displayName":"{name}","fileName":"{name}","releaseType":1,
            "hashes":[{{"value":"{}","algo":1}}],"fileDate":"2026-10-01T00:00:00Z","fileLength":{},
            "downloadUrl":{url},"gameVersions":["1.21.1","Fabric"],
            "sortableGameVersions":[{{"gameVersion":"1.21.1"}}],"dependencies":[],
            "fileFingerprint":{}}}"#,
        sha1_hex(body),
        body.len(),
        murmur(body)
    )
}

fn cf_mod(id: u64, name: &str, class: u32) -> String {
    format!(
        r#"{{"id":{id},"name":"{name}","classId":{class},"authors":[{{"name":"maker"}}],
            "links":{{"websiteUrl":"https://www.curseforge.com/minecraft/mc-mods/m{id}"}},
            "latestFilesIndexes":[]}}"#
    )
}

fn modrinth_version(project: &str, name: &str, body: &[u8], url: &str) -> String {
    format!(
        r#"{{"id":"v-{project}","project_id":"{project}","name":"{name}","version_number":"1",
            "version_type":"release","game_versions":["1.21.1"],"loaders":["fabric"],
            "date_published":"2026-01-01T00:00:00Z",
            "files":[{{"url":"{url}","filename":"{name}","primary":true,"size":{},
                       "hashes":{{"sha1":"{}"}}}}],"dependencies":[]}}"#,
        body.len(),
        sha1_hex(body)
    )
}

/// The body of the first request sent to `url`.
fn sent_body(world: &World, url: &str) -> String {
    world
        .net
        .sent
        .lock()
        .unwrap()
        .iter()
        .filter(|request| request.url == url)
        .find_map(|request| request.body.clone())
        .map(|body| String::from_utf8(body).unwrap())
        .unwrap_or_default()
}

#[tokio::test]
async fn the_sources_are_listed_with_modrinth_first() {
    let world = two_source_world();
    let sources = world.service.content_sources().await;
    let ids: Vec<&str> = sources.iter().map(|source| source.id.as_str()).collect();
    assert_eq!(ids, vec![DEFAULT_SOURCE, CF]);
    assert_eq!(sources[1].name, "CurseForge");
    assert_eq!(
        sources[1].catalog,
        Some(crate::content::discover::Catalog::CurseForge)
    );
}

#[tokio::test]
async fn a_file_belongs_to_the_first_source_that_knows_it() {
    let world = two_source_world();
    let record = fabric_instance(&world, "1.21.1").await;
    let mods = world.service.layout.game(&record.id).join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    let (on_modrinth, on_curseforge, nowhere) = (b"modrinth jar", b"curse  jar\n", b"own jar");
    std::fs::write(mods.join("a.jar"), on_modrinth).unwrap();
    std::fs::write(mods.join("b.jar"), on_curseforge).unwrap();
    std::fs::write(mods.join("c.jar"), nowhere).unwrap();

    world
        .net
        .answer("api.modrinth.com/v2/version_files/update", "{}");
    world.net.answer(
        "api.modrinth.com/v2/version_files",
        format!(
            r#"{{"{}":{}}}"#,
            sha1_hex(on_modrinth),
            modrinth_version("MR", "a.jar", on_modrinth, "https://cdn.modrinth.com/a.jar")
        ),
    );
    world.net.answer(
        "api.modrinth.com/v2/projects",
        r#"[{"id":"MR","slug":"from-modrinth","title":"From Modrinth","project_type":"mod"}]"#,
    );
    world.net.answer("api.modrinth.com/v2/teams", "[]");
    world.net.answer(
        "api.curseforge.com/v1/fingerprints/432",
        format!(
            r#"{{"data":{{"exactMatches":[{{"id":42,"file":{}}}]}}}}"#,
            cf_file(
                7,
                42,
                "b.jar",
                on_curseforge,
                "https://edge.forgecdn.net/files/7/b.jar"
            )
        ),
    );
    world.net.answer(
        "api.curseforge.com/v1/mods",
        format!(r#"{{"data":[{}]}}"#, cf_mod(42, "From CurseForge", 6)),
    );

    let list = world
        .service
        .content_details(&record.id, ProjectKind::Mod, ContentLookup::Fresh)
        .await
        .unwrap();
    assert!(!list.sources_unavailable, "{:?}", list.source_note);
    let by_file: BTreeMap<&str, Option<(&str, &str)>> = list
        .entries
        .iter()
        .map(|entry| {
            (
                entry.item.file_name.as_str(),
                entry
                    .source
                    .as_ref()
                    .map(|source| (source.source.as_str(), source.title.as_str())),
            )
        })
        .collect();
    assert_eq!(by_file["a.jar"], Some((DEFAULT_SOURCE, "From Modrinth")));
    assert_eq!(by_file["b.jar"], Some((CF, "From CurseForge")));
    assert_eq!(by_file["c.jar"], None);
    let b = list
        .entries
        .iter()
        .find(|entry| entry.item.file_name == "b.jar")
        .unwrap();
    assert_eq!(
        b.source.as_ref().unwrap().page_url,
        "https://www.curseforge.com/minecraft/mc-mods/m42"
    );
    // Only what Modrinth did not know went on to CurseForge.
    let asked = sent_body(&world, &format!("{CF_API}/v1/fingerprints/432"));
    let mut expected = [murmur(on_curseforge), murmur(nowhere)];
    expected.sort_unstable();
    assert_eq!(
        asked,
        format!(r#"{{"fingerprints":[{},{}]}}"#, expected[0], expected[1])
    );
}

#[tokio::test]
async fn a_source_that_cannot_answer_leaves_the_next_one_to_try() {
    let world = two_source_world();
    let record = fabric_instance(&world, "1.21.1").await;
    let mods = world.service.layout.game(&record.id).join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("b.jar"), b"curse").unwrap();
    // Modrinth is unreachable; CurseForge still names the file.
    world.net.answer(
        "api.curseforge.com/v1/fingerprints/432",
        format!(
            r#"{{"data":{{"exactMatches":[{{"id":42,"file":{}}}]}}}}"#,
            cf_file(
                7,
                42,
                "b.jar",
                b"curse",
                "https://edge.forgecdn.net/files/7/b.jar"
            )
        ),
    );
    world.net.answer(
        "api.curseforge.com/v1/mods",
        format!(r#"{{"data":[{}]}}"#, cf_mod(42, "From CurseForge", 6)),
    );
    let list = world
        .service
        .content_details(&record.id, ProjectKind::Mod, ContentLookup::Fresh)
        .await
        .unwrap();
    assert!(list.sources_unavailable);
    assert!(list.source_note.as_deref().unwrap().contains("Modrinth"));
    assert_eq!(list.entries[0].source.as_ref().unwrap().source, CF);
}

/// A CurseForge mod whose only file its author lets people download only
/// on CurseForge's site.
fn blocked_project(world: &World, body: &[u8]) {
    world.net.answer(
        "api.curseforge.com/v1/mods/42/files?index=0",
        format!(
            r#"{{"data":[{}],"pagination":{{"totalCount":1}}}}"#,
            cf_file(7, 42, "blocked.jar", body, "")
        ),
    );
    world.net.answer(
        "api.curseforge.com/v1/mods/42",
        format!(r#"{{"data":{}}}"#, cf_mod(42, "Blocked", 6)),
    );
}

#[tokio::test]
async fn a_file_curseforge_may_not_hand_out_comes_from_modrinth_when_it_is_there() {
    let world = two_source_world();
    let record = fabric_instance(&world, "1.21.1").await;
    let body = b"the same jar";
    blocked_project(&world, body);
    world.net.answer(
        "api.modrinth.com/v2/version_files",
        format!(
            r#"{{"{}":{}}}"#,
            sha1_hex(body),
            modrinth_version(
                "MR",
                "same.jar",
                body,
                "https://cdn.modrinth.com/data/same.jar"
            )
        ),
    );
    world
        .net
        .answer("cdn.modrinth.com/data/same.jar", body.to_vec());
    let name = world
        .service
        .install_content(
            &record.id,
            ProjectKind::Mod,
            CF,
            "42",
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(name, "blocked.jar", "named as CurseForge names it");
    let installed = world
        .service
        .layout
        .game(&record.id)
        .join("mods/blocked.jar");
    assert_eq!(
        std::fs::read(installed).unwrap(),
        body,
        "only Modrinth's address serves these bytes"
    );
    assert!(
        sent_body(&world, "https://api.modrinth.com/v2/version_files").contains(&sha1_hex(body))
    );
}

#[tokio::test]
async fn a_file_nobody_may_hand_out_names_its_download_page() {
    let world = two_source_world();
    let record = fabric_instance(&world, "1.21.1").await;
    blocked_project(&world, b"only on the site");
    world.net.answer("api.modrinth.com/v2/version_files", "{}");
    let error = world
        .service
        .install_content(
            &record.id,
            ProjectKind::Mod,
            CF,
            "42",
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    match error {
        ServiceError::ManualDownload { file, page } => {
            assert_eq!(file, "blocked.jar");
            assert_eq!(
                page.as_deref(),
                Some("https://www.curseforge.com/minecraft/mc-mods/m42/files/7")
            );
        }
        other => panic!("expected a manual download, got {other:?}"),
    }
    assert!(
        !world
            .service
            .layout
            .game(&record.id)
            .join("mods/blocked.jar")
            .exists(),
        "nothing half-made is left"
    );
    let retry = world.service.activity(10).finished.remove(0).retry.unwrap();
    assert!(matches!(
        retry,
        crate::activity::RetryAction::InstallContent { ref source, .. } if source == CF
    ));
}

fn write_curseforge_pack(path: &std::path::Path, manifest: &str) {
    use std::io::Write as _;
    let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    writer.start_file("manifest.json", options).unwrap();
    writer.write_all(manifest.as_bytes()).unwrap();
    writer
        .start_file("overrides/config/pack.txt", options)
        .unwrap();
    writer.write_all(b"configured").unwrap();
    writer.finish().unwrap();
}

#[tokio::test]
async fn a_curseforge_pack_installs_what_it_can_and_lists_the_rest() {
    let world = two_source_world();
    let (free, mirrored, manual) = (
        b"free jar".as_slice(),
        b"mirrored jar".as_slice(),
        b"manual jar".as_slice(),
    );
    world.net.answer(
        "api.curseforge.com/v1/mods/files",
        format!(
            r#"{{"data":[{},{},{}]}}"#,
            cf_file(
                1,
                11,
                "free.jar",
                free,
                "https://edge.forgecdn.net/files/1/free.jar"
            ),
            cf_file(2, 12, "mirrored.jar", mirrored, ""),
            cf_file(3, 13, "manual.jar", manual, "")
        ),
    );
    world.net.answer(
        "api.curseforge.com/v1/mods",
        format!(
            r#"{{"data":[{},{},{}]}}"#,
            cf_mod(11, "Free", 6),
            cf_mod(12, "Mirrored", 6),
            cf_mod(13, "Manual", 6)
        ),
    );
    world.net.answer(
        "api.modrinth.com/v2/version_files",
        format!(
            r#"{{"{}":{}}}"#,
            sha1_hex(mirrored),
            modrinth_version(
                "MR",
                "m.jar",
                mirrored,
                "https://cdn.modrinth.com/data/m.jar"
            )
        ),
    );
    world
        .net
        .answer("edge.forgecdn.net/files/1/free.jar", free.to_vec());
    world
        .net
        .answer("cdn.modrinth.com/data/m.jar", mirrored.to_vec());
    let pack = world.server.join("pack.zip");
    write_curseforge_pack(
        &pack,
        r#"{"minecraft":{"version":"1.21.1","modLoaders":[{"id":"fabric-0.16.5","primary":true}]},
            "manifestType":"minecraftModpack","name":"Curse Pack","version":"1.0",
            "files":[{"projectID":11,"fileID":1,"required":true},
                     {"projectID":12,"fileID":2,"required":false},
                     {"projectID":13,"fileID":3,"required":true}],
            "overrides":"overrides"}"#,
    );
    let record = world
        .service
        .import_modpack_file(&pack, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(record.name, "Curse Pack");
    assert_eq!(
        (record.loader, record.game_version.as_str()),
        (Loader::Fabric, "1.21.1")
    );
    let game = world.service.layout.game(&record.id);
    assert_eq!(std::fs::read(game.join("mods/free.jar")).unwrap(), free);
    assert_eq!(
        std::fs::read(game.join("mods/mirrored.jar.disabled")).unwrap(),
        mirrored,
        "switched off as the pack's author had it, fetched from Modrinth"
    );
    assert_eq!(
        std::fs::read(game.join("config/pack.txt")).unwrap(),
        b"configured"
    );
    assert!(!game.join("mods/manual.jar").exists());

    let pending = world.service.pending_downloads(&record.id).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].title, "Manual");
    assert_eq!(pending[0].path, "mods/manual.jar");
    assert_eq!(
        pending[0].page.as_deref(),
        Some("https://www.curseforge.com/minecraft/mc-mods/m13/files/3")
    );

    // The person downloads it (under another name) and adds it.
    let downloaded = world.server.join("manual (1).jar");
    std::fs::write(&downloaded, manual).unwrap();
    let stranger = world.server.join("other.jar");
    std::fs::write(&stranger, b"something else").unwrap();
    let added = world
        .service
        .add_pending_files(&record.id, vec![stranger, downloaded])
        .await
        .unwrap();
    assert!(matches!(added[0].1, AddedFile::NotWanted));
    assert!(matches!(&added[1].1, AddedFile::Placed(path) if path == "mods/manual.jar"));
    assert_eq!(std::fs::read(game.join("mods/manual.jar")).unwrap(), manual);
    assert!(
        world
            .service
            .pending_downloads(&record.id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        !world
            .service
            .layout
            .profile(&record.id)
            .join("pending-downloads.json")
            .exists(),
        "nothing left to wait for"
    );
}

#[tokio::test]
async fn a_curseforge_pack_needs_a_curseforge_source() {
    let world = world();
    let pack = world.server.join("pack.zip");
    write_curseforge_pack(
        &pack,
        r#"{"minecraft":{"version":"1.21.1"},"name":"Curse Pack","files":[]}"#,
    );
    let error = world
        .service
        .import_modpack_file(&pack, CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        ServiceError::NoCatalogSource(crate::content::discover::Catalog::CurseForge)
    ));
    assert!(world.service.library().await.instances.is_empty());
}

#[tokio::test]
async fn a_pack_installed_from_curseforge_remembers_where_it_came_from() {
    let world = two_source_world();
    let pack = world.server.join("download.zip");
    write_curseforge_pack(
        &pack,
        r#"{"minecraft":{"version":"1.21.1"},"name":"From Discover","files":[]}"#,
    );
    let bytes = std::fs::read(&pack).unwrap();
    world.net.answer(
        "api.curseforge.com/v1/mods/99/files?index=0",
        format!(
            r#"{{"data":[{}],"pagination":{{"totalCount":1}}}}"#,
            cf_file(
                5,
                99,
                "pack.zip",
                &bytes,
                "https://edge.forgecdn.net/files/5/pack.zip"
            )
        ),
    );
    world.net.answer(
        "api.curseforge.com/v1/mods/99",
        format!(r#"{{"data":{}}}"#, cf_mod(99, "From Discover", 4471)),
    );
    world
        .net
        .answer("edge.forgecdn.net/files/5/pack.zip", bytes);
    let record = world
        .service
        .install_modpack(CF, "99", None, CancellationToken::new())
        .await
        .unwrap();
    let origin = ProjectOrigin::parse(record.source_project.as_deref().unwrap());
    assert_eq!(
        (origin.source.as_str(), origin.project.as_str()),
        (CF, "99")
    );
    let stored = world.service.instance(&record.id).await.unwrap();
    assert_eq!(
        stored.source_project.as_deref(),
        Some("lumilio.curseforge/99")
    );
}

#[test]
fn a_modrinth_origin_is_stored_as_before() {
    let modrinth = ProjectOrigin {
        source: DEFAULT_SOURCE.into(),
        project: "AANobbMI".into(),
    };
    assert_eq!(modrinth.stored(), "AANobbMI");
    assert_eq!(ProjectOrigin::parse("AANobbMI"), modrinth);
    let curseforge = ProjectOrigin {
        source: CF.into(),
        project: "238222".into(),
    };
    assert_eq!(ProjectOrigin::parse(&curseforge.stored()), curseforge);
}
