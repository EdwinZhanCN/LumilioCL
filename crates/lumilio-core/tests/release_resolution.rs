use std::path::PathBuf;
use std::str::FromStr;

use lumilio_core::{
    CompatibilityRule, HostProfile, LaunchContext, LaunchDirectories, LaunchError,
    MachineArchitecture, PackageCoordinate, PlatformFamily, ReleaseManifest, ReleaseSet,
    RuleDecision,
};

fn mac_host() -> HostProfile {
    HostProfile::new(PlatformFamily::MacOs, MachineArchitecture::Arm64, "15.6")
}

#[test]
fn package_coordinates_have_stable_repository_paths() {
    let coordinate = PackageCoordinate::from_str("org.example:render-engine:4.2:natives@zip")
        .expect("valid coordinate");

    assert_eq!(coordinate.namespace(), "org.example");
    assert_eq!(coordinate.component(), "render-engine");
    assert_eq!(coordinate.release(), "4.2");
    assert_eq!(coordinate.variant(), Some("natives"));
    assert_eq!(coordinate.extension(), "zip");
    assert_eq!(
        coordinate.repository_path(),
        "org/example/render-engine/4.2/render-engine-4.2-natives.zip"
    );
    assert_eq!(
        coordinate.to_string(),
        "org.example:render-engine:4.2:natives@zip"
    );

    for malformed in [
        "org.example:render-engine",
        "org.example::4.2",
        "org.example:render-engine:4.2@",
        "org.example:render-engine:4.2@zip@tar",
    ] {
        assert!(
            PackageCoordinate::from_str(malformed).is_err(),
            "{malformed}"
        );
    }
}

#[test]
fn the_last_matching_environment_rule_decides() {
    let rules = vec![
        CompatibilityRule::new(RuleDecision::Permit),
        CompatibilityRule::new(RuleDecision::Block).requiring_feature("demo", true),
        CompatibilityRule::new(RuleDecision::Permit).requiring_platform("osx"),
    ];

    let host = mac_host().with_feature("demo", true);
    assert!(host.allows(&rules));

    let only_non_matching_rule =
        vec![CompatibilityRule::new(RuleDecision::Permit).requiring_feature("missing", true)];
    assert!(!host.allows(&only_non_matching_rule));
    assert!(host.allows(&[]));
}

#[test]
fn release_json_supports_rule_gated_multi_value_arguments() {
    let release = ReleaseManifest::decode_json(
        r#"{
          "id": "current",
          "mainClass": "example.client.EntryPoint",
          "javaVersion": { "component": "java-runtime-delta", "majorVersion": 21 },
          "arguments": {
            "jvm": ["-Dplain=${plain}"],
            "game": [
              "--username",
              "${auth_player_name}",
              {
                "rules": [{ "action": "allow", "features": { "custom_size": true } }],
                "values": ["--width", "${resolution_width}", "--height", "${resolution_height}"]
              }
            ]
          }
        }"#,
    )
    .expect("valid release JSON");

    let context = LaunchContext::new(mac_host(), LaunchDirectories::under("/launcher"))
        .with_value("plain", "kept")
        .with_value("auth_player_name", "Alex")
        .with_value("resolution_width", "1920")
        .with_value("resolution_height", "1080")
        .with_feature("custom_size", true);
    let plan = release
        .build_launch_plan(&context)
        .expect("launch specification");

    assert!(
        plan.jvm_arguments()
            .iter()
            .any(|value| value == "-Dplain=kept")
    );
    assert_eq!(
        plan.game_arguments(),
        ["--username", "Alex", "--width", "1920", "--height", "1080"]
    );
    assert_eq!(
        plan.java_requirement().expect("Java requirement").major(),
        21
    );
}

#[test]
fn legacy_arguments_are_tokenized_without_losing_quoted_values() {
    let release = ReleaseManifest::decode_json(
        r#"{
          "id": "legacy",
          "mainClass": "example.legacy.EntryPoint",
          "minecraftArguments": "--username ${auth_player_name} --gameDir \"${game_directory}\""
        }"#,
    )
    .expect("valid release JSON");

    let context = LaunchContext::new(mac_host(), LaunchDirectories::under("/launcher"))
        .with_value("auth_player_name", "Alex")
        .with_value("game_directory", "/Games/My World");
    let plan = release
        .build_launch_plan(&context)
        .expect("launch specification");

    assert_eq!(
        plan.game_arguments(),
        ["--username", "Alex", "--gameDir", "/Games/My World"]
    );
}

#[test]
fn inheritance_is_resolved_once_with_stable_argument_and_library_order() {
    let parent = ReleaseManifest::decode_json(
        r#"{
          "id": "base",
          "mainClass": "example.Base",
          "minimumLauncherVersion": 10,
          "arguments": { "game": ["--from-parent"], "jvm": [] },
          "libraries": [{ "name": "org.example:parent:1.0" }]
        }"#,
    )
    .expect("parent");
    let child = ReleaseManifest::decode_json(
        r#"{
          "id": "modded",
          "inheritsFrom": "base",
          "minimumLauncherVersion": 7,
          "arguments": { "game": ["--from-child"] },
          "libraries": [{ "name": "org.example:child:2.0" }]
        }"#,
    )
    .expect("child");

    let mut releases = ReleaseSet::new();
    releases.insert(parent);
    releases.insert(child);
    let resolved = releases.resolve("modded").expect("resolved release");

    assert_eq!(resolved.parent_id(), None);
    assert_eq!(resolved.main_class(), Some("example.Base"));
    assert_eq!(resolved.minimum_launcher_revision(), Some(10));
    assert_eq!(
        resolved
            .libraries()
            .iter()
            .map(|library| library.coordinate().component())
            .collect::<Vec<_>>(),
        ["child", "parent"]
    );

    let plan = resolved
        .build_launch_plan(&LaunchContext::new(
            mac_host(),
            LaunchDirectories::under("/launcher"),
        ))
        .expect("launch specification");
    assert_eq!(plan.game_arguments(), ["--from-parent", "--from-child"]);
}

#[test]
fn launch_plan_separates_classpath_entries_from_native_archives() {
    let release = ReleaseManifest::decode_json(
        r#"{
          "id": "native-test",
          "mainClass": "example.client.EntryPoint",
          "arguments": { "jvm": ["-cp", "${classpath}"], "game": [] },
          "libraries": [
            {
              "name": "org.example:engine:1.0",
              "downloads": {
                "artifact": {
                  "path": "custom/engine.jar",
                  "url": "https://cdn.example/engine.jar",
                  "sha1": "abc",
                  "size": 42
                }
              }
            },
            {
              "name": "org.example:engine-native:1.0",
              "natives": { "osx": "natives-macos-${arch}" },
              "downloads": {
                "classifiers": {
                  "natives-macos-64": {
                    "path": "custom/engine-native.jar",
                    "url": "https://cdn.example/engine-native.jar"
                  }
                }
              }
            }
          ]
        }"#,
    )
    .expect("release");

    let plan = release
        .build_launch_plan(&LaunchContext::new(
            mac_host(),
            LaunchDirectories::under("/launcher"),
        ))
        .expect("launch specification");

    assert_eq!(
        plan.classpath(),
        [
            PathBuf::from("/launcher/libraries/custom/engine.jar"),
            PathBuf::from("/launcher/versions/native-test/native-test.jar"),
        ]
    );
    assert_eq!(
        plan.native_archives()[0].path(),
        PathBuf::from("/launcher/libraries/custom/engine-native.jar")
    );
    assert_eq!(
        plan.jvm_arguments(),
        [
            "-cp",
            "/launcher/libraries/custom/engine.jar:/launcher/versions/native-test/native-test.jar"
        ]
    );
}

#[test]
fn protocol_round_trip_preserves_extension_data_and_download_metadata() {
    let release = ReleaseManifest::decode_json(
        r#"{
          "id": "metadata-test",
          "mainClass": "example.EntryPoint",
          "assetIndex": {
            "id": "22",
            "url": "https://cdn.example/assets.json",
            "sha1": "asset-sha1",
            "size": 100,
            "totalSize": 200
          },
          "downloads": {
            "client": {
              "url": "https://cdn.example/client.jar",
              "sha1": "client-sha1",
              "size": 300
            }
          },
          "logging": {
            "client": {
              "argument": "-Dlog.config=${path}",
              "file": {
                "id": "client.xml",
                "url": "https://cdn.example/client.xml",
                "sha1": "log-sha1",
                "size": 400
              },
              "type": "log4j2-xml"
            }
          },
          "customExtension": { "kept": true }
        }"#,
    )
    .expect("metadata release");

    let encoded: serde_json::Value =
        serde_json::from_str(&release.encode_json().expect("encoded release")).unwrap();
    assert_eq!(encoded["customExtension"]["kept"], true);
    assert_eq!(encoded["assetIndex"]["totalSize"], 200);
    assert_eq!(encoded["downloads"]["client"]["sha1"], "client-sha1");
    assert_eq!(encoded["logging"]["client"]["file"]["id"], "client.xml");

    let plan = release
        .build_launch_plan(&LaunchContext::new(
            mac_host(),
            LaunchDirectories::under("/launcher"),
        ))
        .expect("launch plan");
    assert_eq!(plan.asset_catalog().expect("asset index").id(), "22");
    assert_eq!(plan.required_downloads().len(), 1);
    assert_eq!(
        plan.required_downloads()[0].destination(),
        PathBuf::from("/launcher/versions/metadata-test/metadata-test.jar")
    );
}

#[test]
fn launch_plan_rejects_unresolved_variables() {
    let release = ReleaseManifest::decode_json(
        r#"{
          "id": "missing-variable",
          "mainClass": "example.EntryPoint",
          "arguments": { "jvm": ["-Drequired=${missing}"], "game": [] }
        }"#,
    )
    .unwrap();

    let error = release
        .build_launch_plan(&LaunchContext::new(
            mac_host(),
            LaunchDirectories::under("/launcher"),
        ))
        .unwrap_err();

    assert_eq!(error, LaunchError::MissingVariable("missing".to_owned()));
}

#[test]
fn repeated_and_blocked_libraries_do_not_duplicate_launch_inputs() {
    let release = ReleaseManifest::decode_json(
        r#"{
          "id": "deduplicated",
          "mainClass": "example.EntryPoint",
          "arguments": { "jvm": [], "game": [] },
          "libraries": [
            { "name": "org.example:shared:1.0" },
            { "name": "org.example:shared:1.0" },
            {
              "name": "org.example:blocked:1.0",
              "rules": [{ "action": "disallow" }]
            }
          ]
        }"#,
    )
    .unwrap();

    let plan = release
        .build_launch_plan(&LaunchContext::new(
            mac_host(),
            LaunchDirectories::under("/launcher"),
        ))
        .unwrap();

    assert_eq!(plan.classpath().len(), 2);
    assert_eq!(plan.required_downloads().len(), 1);
}
