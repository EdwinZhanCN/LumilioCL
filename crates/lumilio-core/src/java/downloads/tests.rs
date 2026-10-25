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
fn apple_silicon_falls_back_to_the_intel_runtime_only_where_no_native_one_exists() {
    let entry = |name: &str| {
        format!(
            r#"[{{"manifest":{{"url":"https://launchermeta.mojang.com/{name}","sha1":"aa","size":1}},"version":{{"name":"{name}"}}}}]"#
        )
    };
    let index = format!(
        r#"{{"mac-os-arm64":{{"java-runtime-delta":{d}}},
           "mac-os":{{"jre-legacy":{l},"java-runtime-delta":{d},"minecraft-java-exe":{exe}}}}}"#,
        d = entry("21.0.7"),
        l = entry("8u74"),
        exe = entry("14"),
    );
    let arm = HostProfile::new(PlatformFamily::MacOs, MachineArchitecture::Arm64, "");
    assert_eq!(platform_keys(&arm), ["mac-os-arm64", "mac-os"]);
    assert_eq!(
        choose_for_host(&index, &arm, Some(8), JavaBuild::Any)
            .unwrap()
            .component,
        "jre-legacy"
    );
    let native = choose_for_host(&index, &arm, Some(21), JavaBuild::Any).unwrap();
    assert_eq!(
        native.manifest.url,
        "https://launchermeta.mojang.com/21.0.7"
    );
    assert_eq!(
        choose_for_host(&index, &arm, Some(17), JavaBuild::Any),
        Err(JavaRuntimeError::NoSuchRuntime(Some(17)))
    );
    // The launcher helper in the index is not a Java.
    assert_eq!(offered_majors(&index, &arm).unwrap(), [8, 21]);
    let intel = HostProfile::new(PlatformFamily::MacOs, MachineArchitecture::X86_64, "");
    assert_eq!(platform_keys(&intel), ["mac-os"]);
    let riscv = HostProfile::new(
        PlatformFamily::Linux,
        MachineArchitecture::Other("riscv64".into()),
        "",
    );
    assert_eq!(
        choose_for_host(&index, &riscv, Some(21), JavaBuild::Any),
        Err(JavaRuntimeError::UnsupportedPlatform)
    );
}

#[test]
fn an_intel_build_comes_only_from_the_intel_runtimes_into_its_own_folder() {
    let entry = |platform: &str, name: &str| {
        format!(
            r#"[{{"manifest":{{"url":"https://launchermeta.mojang.com/{platform}/{name}","sha1":"aa","size":1}},"version":{{"name":"{name}"}}}}]"#
        )
    };
    let index = format!(
        r#"{{"mac-os-arm64":{{"java-runtime-gamma":{arm_gamma},"java-runtime-delta":{arm_delta}}},
           "mac-os":{{"jre-legacy":{legacy},"java-runtime-gamma":{gamma},"java-runtime-delta":{delta}}},
           "windows-arm64":{{"java-runtime-gamma":{arm_gamma}}},
           "windows-x64":{{"java-runtime-gamma":{win_gamma}}}}}"#,
        arm_gamma = entry("mac-os-arm64", "17.0.15"),
        arm_delta = entry("mac-os-arm64", "21.0.7"),
        legacy = entry("mac-os", "1.8.0_74"),
        gamma = entry("mac-os", "17.0.15"),
        delta = entry("mac-os", "21.0.7"),
        win_gamma = entry("windows-x64", "17.0.15"),
    );
    let mac_arm = HostProfile::new(PlatformFamily::MacOs, MachineArchitecture::Arm64, "");
    let pick = |host: &HostProfile, wanted, build| {
        choose_for_host(&index, host, wanted, build)
            .map(|choice| (choice.manifest.url.clone(), choice.folder(host)))
            .unwrap()
    };
    let at =
        |platform: &str, name: &str| format!("https://launchermeta.mojang.com/{platform}/{name}");
    // Java 17 exists for both: Any takes the native one, Intel the other,
    // and the two do not share a folder.
    assert_eq!(
        pick(&mac_arm, Some(17), JavaBuild::Any),
        (
            at("mac-os-arm64", "17.0.15"),
            "java-runtime-gamma".to_owned()
        )
    );
    assert_eq!(
        pick(&mac_arm, Some(17), JavaBuild::Intel),
        (
            at("mac-os", "17.0.15"),
            "java-runtime-gamma-mac-os".to_owned()
        )
    );
    assert_eq!(
        pick(&mac_arm, Some(8), JavaBuild::Intel),
        (at("mac-os", "1.8.0_74"), "jre-legacy-mac-os".to_owned())
    );
    let windows_arm = HostProfile::new(PlatformFamily::Windows, MachineArchitecture::Arm64, "");
    assert_eq!(
        pick(&windows_arm, Some(17), JavaBuild::Intel).0,
        at("windows-x64", "17.0.15")
    );
    // On an Intel Mac the Intel runtime is the native one.
    let mac_intel = HostProfile::new(PlatformFamily::MacOs, MachineArchitecture::X86_64, "");
    assert_eq!(
        pick(&mac_intel, Some(17), JavaBuild::Intel),
        (at("mac-os", "17.0.15"), "java-runtime-gamma".to_owned())
    );
    let linux_arm = HostProfile::new(PlatformFamily::Linux, MachineArchitecture::Arm64, "");
    assert_eq!(
        choose_for_host(&index, &linux_arm, Some(17), JavaBuild::Intel),
        Err(JavaRuntimeError::UnsupportedPlatform)
    );
}

#[test]
fn graalvm_is_used_only_for_the_majors_it_offers() {
    let mac = HostProfile::new(PlatformFamily::MacOs, MachineArchitecture::Arm64, "");
    let graal = JavaSource::OracleGraalVm;
    assert_eq!(source_for(graal, &mac, Some(21)), graal);
    assert_eq!(source_for(graal, &mac, None), graal);
    // Old versions need Java 8, which only Mojang has.
    assert_eq!(source_for(graal, &mac, Some(8)), JavaSource::Mojang);
    assert_eq!(
        source_for(JavaSource::Mojang, &mac, Some(21)),
        JavaSource::Mojang
    );
    let windows_arm = HostProfile::new(PlatformFamily::Windows, MachineArchitecture::Arm64, "");
    assert_eq!(source_for(graal, &windows_arm, None), JavaSource::Mojang);
    assert_eq!(graal.folder_name(25).as_deref(), Some("graalvm-25"));
    assert_eq!(JavaSource::Mojang.folder_name(25), None);
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
fn a_link_leads_to_the_file_it_names_for_a_copy_where_links_cannot_be_made() {
    let manifest = r#"{"files":{
        "legal/java.base/LICENSE":{"type":"file","downloads":{
            "raw":{"url":"https://piston-data.mojang.com/v1/objects/x/LICENSE","sha1":"bb"}}},
        "legal/java.sql/LICENSE":{"type":"link","target":"../java.base/LICENSE"},
        "legal/java.xml/LICENSE":{"type":"link","target":"./../java.sql/LICENSE"},
        "legal/java.base":{"type":"directory"},
        "legal/all":{"type":"link","target":"java.base"},
        "loop/a":{"type":"link","target":"b"},
        "loop/b":{"type":"link","target":"a"},
        "gone":{"type":"link","target":"missing"}}}"#;
    let entries = parse_manifest(manifest, is_trusted_source).unwrap();
    for link in ["legal/java.sql/LICENSE", "legal/java.xml/LICENSE"] {
        assert_eq!(
            linked_file(&entries, link).as_deref(),
            Some("legal/java.base/LICENSE"),
            "{link}"
        );
    }
    for unusable in ["legal/all", "loop/a", "gone"] {
        assert_eq!(linked_file(&entries, unusable), None, "{unusable}");
    }
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
