use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use lumilio_core::{
    ArtifactKind, AssetIndex, CancellationToken, FileTransport, HostProfile, InstallationPlan,
    InstallationVerifier, Installer, LaunchContext, LaunchDirectories, MachineArchitecture,
    NativeBundle, NativePublisher, OfficialSource, PlatformFamily, PrefixMirror, ReleaseManifest,
    SourceChain, SourceProvider, TransferEngine,
};
use sha1::{Digest, Sha1};
use tempfile::tempdir;
use url::Url;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

fn linux_host() -> HostProfile {
    HostProfile::new(PlatformFamily::Linux, MachineArchitecture::X86_64, "6.12")
}

fn official_sources() -> SourceChain {
    SourceChain::new([Arc::new(OfficialSource) as Arc<dyn SourceProvider>]).unwrap()
}

fn digest(bytes: &[u8]) -> String {
    Sha1::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn file_url(path: &Path) -> String {
    Url::from_file_path(path).unwrap().to_string()
}

fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let file = std::fs::File::create(path).unwrap();
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, contents) in entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(contents).unwrap();
    }
    writer.finish().unwrap();
}

#[test]
fn asset_catalog_validates_paths_and_deduplicates_content_objects() {
    let hash = "0123456789abcdef0123456789abcdef01234567";
    let index = AssetIndex::decode_json(&format!(
        r#"{{
          "objects": {{
            "sounds/step.ogg": {{ "hash": "{hash}", "size": 12 }},
            "sounds/alias.ogg": {{ "hash": "{hash}", "size": 12 }}
          }},
          "virtual": true,
          "map_to_resources": true
        }}"#
    ))
    .unwrap();

    assert_eq!(index.objects().len(), 2);
    assert_eq!(index.unique_objects().len(), 1);
    let object = &index.unique_objects()[0];
    assert_eq!(
        object.relative_path(),
        Path::new("01/0123456789abcdef0123456789abcdef01234567")
    );
    assert_eq!(
        object.official_url(),
        "https://resources.download.minecraft.net/01/0123456789abcdef0123456789abcdef01234567"
    );
    assert!(index.uses_virtual_layout());
    assert!(index.maps_to_resources());

    let unsafe_index =
        format!(r#"{{"objects": {{"../escape": {{"hash": "{hash}", "size": 1}}}}}}"#);
    assert!(AssetIndex::decode_json(&unsafe_index).is_err());
}

#[test]
fn installation_plan_covers_every_vanilla_artifact_class() {
    let root = tempdir().unwrap();
    let directories = LaunchDirectories::under(root.path());
    let release = ReleaseManifest::decode_json(
        r#"{
          "id": "complete",
          "mainClass": "example.Entry",
          "arguments": { "jvm": [], "game": [] },
          "assetIndex": {
            "id": "catalog-1",
            "url": "https://meta.invalid/catalog.json",
            "sha1": "1111111111111111111111111111111111111111",
            "size": 50
          },
          "downloads": {
            "client": {
              "url": "https://files.invalid/client.jar",
              "sha1": "2222222222222222222222222222222222222222",
              "size": 100
            }
          },
          "logging": {
            "client": {
              "argument": "-Dlog.configurationFile=${path}",
              "file": {
                "id": "client-log.xml",
                "url": "https://files.invalid/client-log.xml",
                "sha1": "3333333333333333333333333333333333333333",
                "size": 20
              },
              "type": "log4j2-xml"
            }
          },
          "libraries": [
            {
              "name": "org.example:regular:1.0",
              "downloads": { "artifact": {
                "path": "org/example/regular/1.0/regular-1.0.jar",
                "url": "https://files.invalid/regular.jar",
                "sha1": "4444444444444444444444444444444444444444",
                "size": 30
              }}
            },
            {
              "name": "org.example:native:1.0",
              "natives": { "linux": "natives-linux" },
              "extract": { "exclude": ["META-INF/"] },
              "downloads": { "classifiers": { "natives-linux": {
                "path": "org/example/native/1.0/native-1.0-natives-linux.jar",
                "url": "https://files.invalid/native.jar",
                "sha1": "5555555555555555555555555555555555555555",
                "size": 40
              }}}
            }
          ]
        }"#,
    )
    .unwrap();
    let context = LaunchContext::new(linux_host(), directories.clone());
    let launch = release.build_launch_plan(&context).unwrap();
    let plan = InstallationPlan::build(
        &release,
        &launch,
        directories,
        official_sources(),
        Some(root.path().join("shared-cache")),
    )
    .unwrap();

    let kinds = plan
        .initial_artifacts()
        .iter()
        .map(|artifact| artifact.kind())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        kinds,
        BTreeSet::from([
            ArtifactKind::Client,
            ArtifactKind::Library,
            ArtifactKind::NativeArchive,
            ArtifactKind::LoggingConfiguration,
            ArtifactKind::AssetCatalog,
        ])
    );
    assert_eq!(plan.native_bundles().len(), 1);
    assert_eq!(
        plan.manifest_destination(),
        root.path().join("versions/complete/complete.json")
    );
}

#[tokio::test]
async fn native_publication_rejects_traversal_and_preserves_live_directory() {
    let temp = tempdir().unwrap();
    let archive = temp.path().join("malicious.zip");
    write_zip(&archive, &[("../escape.bin", b"bad")]);
    let live = temp.path().join("natives");
    std::fs::create_dir_all(&live).unwrap();
    std::fs::write(live.join("previous.bin"), b"kept").unwrap();

    let error = NativePublisher::default()
        .publish(
            &[NativeBundle::new(&archive)],
            &live,
            CancellationToken::new(),
        )
        .await
        .unwrap_err();

    assert!(error.to_string().contains("unsafe archive path"));
    assert_eq!(std::fs::read(live.join("previous.bin")).unwrap(), b"kept");
    assert!(!temp.path().join("escape.bin").exists());
}

#[tokio::test]
async fn native_publication_honors_exclusions_and_replaces_as_one_unit() {
    let temp = tempdir().unwrap();
    let archive = temp.path().join("natives.zip");
    write_zip(
        &archive,
        &[
            ("META-INF/signature.SF", b"signature"),
            ("linux/libexample.so", b"native"),
        ],
    );
    let live = temp.path().join("natives");
    std::fs::create_dir_all(&live).unwrap();
    std::fs::write(live.join("obsolete.bin"), b"old").unwrap();

    let report = NativePublisher::default()
        .publish(
            &[NativeBundle::new(&archive).excluding(["META-INF/"])],
            &live,
            CancellationToken::new(),
        )
        .await
        .unwrap();

    assert_eq!(report.published_files(), 1);
    assert_eq!(
        std::fs::read(live.join("linux/libexample.so")).unwrap(),
        b"native"
    );
    assert!(!live.join("META-INF/signature.SF").exists());
    assert!(!live.join("obsolete.bin").exists());
}

#[tokio::test]
async fn verification_builds_a_minimal_repair_plan() {
    let temp = tempdir().unwrap();
    let directories = LaunchDirectories::under(temp.path());
    let client = b"valid-client";
    let library = b"valid-library";
    let asset = b"valid-asset";
    let asset_hash = digest(asset);
    let index = AssetIndex::decode_json(&format!(
        r#"{{"objects": {{"icons/main.png": {{"hash": "{asset_hash}", "size": {}}}}}}}"#,
        asset.len()
    ))
    .unwrap();
    let release = ReleaseManifest::decode_json(&format!(
        r#"{{
          "id": "repairable",
          "mainClass": "example.Entry",
          "arguments": {{ "jvm": [], "game": [] }},
          "downloads": {{"client": {{
            "url": "https://files.invalid/client.jar",
            "sha1": "{}", "size": {}
          }}}},
          "libraries": [{{
            "name": "org.example:library:1.0",
            "downloads": {{"artifact": {{
              "path": "org/example/library/1.0/library-1.0.jar",
              "url": "https://files.invalid/library.jar",
              "sha1": "{}", "size": {}
            }}}}
          }}]
        }}"#,
        digest(client),
        client.len(),
        digest(library),
        library.len()
    ))
    .unwrap();
    let launch = release
        .build_launch_plan(&LaunchContext::new(linux_host(), directories.clone()))
        .unwrap();
    let plan =
        InstallationPlan::build(&release, &launch, directories, official_sources(), None).unwrap();

    let client_path = plan
        .initial_artifacts()
        .iter()
        .find(|artifact| artifact.kind() == ArtifactKind::Client)
        .unwrap()
        .request()
        .destination();
    std::fs::create_dir_all(client_path.parent().unwrap()).unwrap();
    std::fs::write(client_path, client).unwrap();
    let library_path = plan
        .initial_artifacts()
        .iter()
        .find(|artifact| artifact.kind() == ArtifactKind::Library)
        .unwrap()
        .request()
        .destination();
    std::fs::create_dir_all(library_path.parent().unwrap()).unwrap();
    std::fs::write(library_path, b"corrupt-library").unwrap();

    let repairs = InstallationVerifier::scan(&plan, Some(&index))
        .await
        .unwrap();
    assert_eq!(repairs.findings().len(), 2);
    assert_eq!(repairs.artifacts().len(), 2);
    let repair_kinds = repairs
        .artifacts()
        .iter()
        .map(|artifact| artifact.kind())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        repair_kinds,
        BTreeSet::from([ArtifactKind::Library, ArtifactKind::AssetObject])
    );
}

#[tokio::test]
async fn installer_completes_an_offline_content_addressed_install() {
    let temp = tempdir().unwrap();
    let sources_root = temp.path().join("sources");
    let install_root = temp.path().join("install");
    std::fs::create_dir_all(&sources_root).unwrap();
    let client = b"offline-client";
    let asset = b"offline-asset";
    let asset_hash = digest(asset);
    let object_source = sources_root.join(format!("assets/{}/{asset_hash}", &asset_hash[..2]));
    std::fs::create_dir_all(object_source.parent().unwrap()).unwrap();
    std::fs::write(&object_source, asset).unwrap();
    let client_source = sources_root.join("client.jar");
    std::fs::write(&client_source, client).unwrap();
    let catalog_json = format!(
        r#"{{"objects": {{"sounds/offline.ogg": {{"hash": "{asset_hash}", "size": {}}}}}, "virtual": true}}"#,
        asset.len()
    );
    let catalog_source = sources_root.join("catalog.json");
    std::fs::write(&catalog_source, &catalog_json).unwrap();

    let release = ReleaseManifest::decode_json(&format!(
        r#"{{
          "id": "offline",
          "mainClass": "example.Entry",
          "arguments": {{"jvm": [], "game": []}},
          "assetIndex": {{
            "id": "offline-assets", "url": "{}",
            "sha1": "{}", "size": {}
          }},
          "downloads": {{"client": {{
            "url": "{}", "sha1": "{}", "size": {}
          }}}}
        }}"#,
        file_url(&catalog_source),
        digest(catalog_json.as_bytes()),
        catalog_json.len(),
        file_url(&client_source),
        digest(client),
        client.len()
    ))
    .unwrap();
    let directories = LaunchDirectories::under(&install_root);
    let launch = release
        .build_launch_plan(&LaunchContext::new(linux_host(), directories.clone()))
        .unwrap();
    let asset_mirror = format!(
        "{}/",
        Url::from_directory_path(sources_root.join("assets"))
            .unwrap()
            .as_str()
            .trim_end_matches('/')
    );
    let sources = SourceChain::new([
        Arc::new(
            PrefixMirror::new("https://resources.download.minecraft.net/", asset_mirror, 4)
                .unwrap(),
        ) as Arc<dyn SourceProvider>,
        Arc::new(OfficialSource) as Arc<dyn SourceProvider>,
    ])
    .unwrap();
    let plan = InstallationPlan::build(&release, &launch, directories, sources, None).unwrap();

    let first = Installer::new(TransferEngine::new(FileTransport, 4).unwrap());
    let second = Installer::new(TransferEngine::new(FileTransport, 4).unwrap());
    let (first, second) = tokio::join!(
        first.execute(&plan, CancellationToken::new()),
        second.execute(&plan, CancellationToken::new()),
    );
    let report = first.unwrap();
    assert_eq!(second.unwrap().asset_objects(), 1);

    assert_eq!(report.asset_objects(), 1);
    assert_eq!(
        std::fs::read(install_root.join("versions/offline/offline.jar")).unwrap(),
        client
    );
    assert_eq!(
        std::fs::read(
            install_root.join(format!("assets/objects/{}/{asset_hash}", &asset_hash[..2]))
        )
        .unwrap(),
        asset
    );
    assert_eq!(
        std::fs::read(install_root.join("assets/virtual/offline-assets/sounds/offline.ogg"))
            .unwrap(),
        asset
    );
    let persisted = std::fs::read_to_string(plan.manifest_destination()).unwrap();
    assert_eq!(
        ReleaseManifest::decode_json(&persisted).unwrap().id(),
        "offline"
    );
}
