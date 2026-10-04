use super::*;

fn index() -> String {
    let entry = |name: &str, url: &str| {
        format!(
            r#"[{{"manifest":{{"url":"{url}","sha1":"aa","size":10}},"version":{{"name":"{name}"}}}}]"#
        )
    };
    format!(
        r#"{{"mac-os-arm64":{{
            "jre-legacy":{legacy},
            "java-runtime-beta":{beta},
            "java-runtime-gamma":{gamma},
            "java-runtime-gamma-snapshot":{gamma},
            "java-runtime-delta":{delta}}},
           "linux":{{"jre-legacy":{legacy}}}}}"#,
        legacy = entry("8.0.202", "https://launchermeta.mojang.com/l"),
        beta = entry("17.0.1", "https://launchermeta.mojang.com/b"),
        gamma = entry("17.0.8", "https://launchermeta.mojang.com/g"),
        delta = entry("21.0.3", "https://launchermeta.mojang.com/d"),
    )
}

#[test]
fn the_wanted_major_gets_its_newest_component_and_no_wish_means_java_21() {
    let pick = |wanted| choose_component(&index(), "mac-os-arm64", wanted);
    assert_eq!(
        pick(Some(17)).unwrap().component,
        "java-runtime-gamma",
        "not the snapshot copy with the same version"
    );
    assert_eq!(pick(Some(8)).unwrap().component, "jre-legacy");
    assert_eq!(pick(Some(21)).unwrap().major, 21);
    assert_eq!(pick(None).unwrap().component, "java-runtime-delta");
    assert_eq!(
        pick(Some(25)),
        Err(JavaRuntimeError::NoSuchRuntime(Some(25)))
    );
    // Only Java 8 offered on this platform: that is the newest there is.
    assert_eq!(
        choose_component(&index(), "linux", None).unwrap().component,
        "jre-legacy"
    );
    assert_eq!(
        choose_component(&index(), "plan9", None),
        Err(JavaRuntimeError::UnsupportedPlatform)
    );
    assert!(matches!(
        choose_component("not json", "linux", None),
        Err(JavaRuntimeError::Invalid(_))
    ));
}

#[test]
fn systems_map_to_the_index_names() {
    let host = |platform, arch| HostProfile::new(platform, arch, "");
    assert_eq!(
        platform_key(&host(PlatformFamily::MacOs, MachineArchitecture::Arm64)),
        Some("mac-os-arm64")
    );
    assert_eq!(
        platform_key(&host(PlatformFamily::Windows, MachineArchitecture::X86_64)),
        Some("windows-x64")
    );
    assert_eq!(
        platform_key(&host(PlatformFamily::Linux, MachineArchitecture::Arm64)),
        None
    );
}

#[test]
fn a_manifest_is_read_and_anything_that_could_escape_is_refused() {
    let good = r#"{"files":{
        "bin":{"type":"directory"},
        "bin/java":{"type":"file","executable":true,"downloads":{
            "raw":{"url":"https://piston-data.mojang.com/v1/objects/x/java","sha1":"bb","size":3}}},
        "lib/jspawnhelper":{"type":"link","target":"../bin/java"}}}"#;
    let entries = parse_manifest(good, is_trusted_source).unwrap();
    assert_eq!(entries.len(), 3);
    assert!(entries.iter().any(|(path, entry)| path == "bin/java"
        && matches!(
            entry,
            Entry::File {
                executable: true,
                ..
            }
        )));

    let bad = |text: &str| parse_manifest(text, is_trusted_source).unwrap_err();
    assert!(matches!(
        bad(r#"{"files":{"../out":{"type":"directory"}}}"#),
        JavaRuntimeError::Unsafe(_)
    ));
    assert!(matches!(
        bad(
            r#"{"files":{"a":{"type":"file","downloads":{"raw":{"url":"https://evil.example/a","sha1":"b"}}}}}"#
        ),
        JavaRuntimeError::Unsafe(_)
    ));
    assert!(matches!(
        bad(r#"{"files":{"a":{"type":"link","target":"../../etc/passwd"}}}"#),
        JavaRuntimeError::Unsafe(_)
    ));
    assert!(matches!(
        bad(r#"{"files":{"a":{"type":"link","target":"/etc/passwd"}}}"#),
        JavaRuntimeError::Unsafe(_)
    ));
    assert!(matches!(
        bad(r#"{"files":{"a":{"type":"file"}}}"#),
        JavaRuntimeError::Invalid(_)
    ));
    assert!(matches!(
        bad(r#"{"files":{"a":{"type":"weird"}}}"#),
        JavaRuntimeError::Invalid(_)
    ));
}

#[test]
fn a_mac_bundle_loses_its_bundle_folder_so_discovery_finds_contents_home() {
    let folder = Path::new("/r/java-runtime-gamma");
    assert_eq!(
        place(folder, "jre.bundle/Contents/Home/bin/java"),
        Path::new("/r/java-runtime-gamma/Contents/Home/bin/java")
    );
    assert_eq!(
        place(folder, "bin/java"),
        Path::new("/r/java-runtime-gamma/bin/java")
    );
}
