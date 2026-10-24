use super::*;

fn host(platform: PlatformFamily, architecture: MachineArchitecture) -> HostProfile {
    HostProfile::new(platform, architecture, "")
}

#[test]
fn addresses_follow_oracles_latest_layout() {
    let mac = archive(&host(PlatformFamily::MacOs, MachineArchitecture::Arm64), 21).unwrap();
    assert_eq!(
        mac.url,
        "https://download.oracle.com/graalvm/21/latest/graalvm-jdk-21_macos-aarch64_bin.tar.gz"
    );
    assert_eq!(mac.checksum_url, format!("{}.sha256", mac.url));
    assert_eq!(mac.kind, ArchiveKind::TarGz);
    assert!(is_trusted_source(&mac.url) && is_trusted_source(&mac.checksum_url));

    let windows = archive(
        &host(PlatformFamily::Windows, MachineArchitecture::X86_64),
        25,
    )
    .unwrap();
    assert_eq!(
        windows.url,
        "https://download.oracle.com/graalvm/25/latest/graalvm-jdk-25_windows-x64_bin.zip"
    );
    assert_eq!(windows.kind, ArchiveKind::Zip);
}

#[test]
fn only_published_majors_and_platforms_are_offered() {
    let linux = host(PlatformFamily::Linux, MachineArchitecture::X86_64);
    assert_eq!(offered_majors(&linux), [21, 25]);
    assert_eq!(
        archive(&linux, 17),
        Err(JavaRuntimeError::NoSuchRuntime(Some(17)))
    );
    let windows_arm = host(PlatformFamily::Windows, MachineArchitecture::Arm64);
    assert!(offered_majors(&windows_arm).is_empty());
    assert_eq!(
        archive(&windows_arm, 21),
        Err(JavaRuntimeError::UnsupportedPlatform)
    );
}

#[test]
fn checksum_files_and_hosts_are_checked() {
    let digest = "A98948C3A1AD037FE2DDE5C80B06B214A5F80C120D8FC2453D155508CF677BEB";
    assert_eq!(
        parse_checksum(&format!("{digest}  graalvm.tar.gz\n")).unwrap(),
        digest.to_ascii_lowercase()
    );
    assert!(parse_checksum("<html>not found</html>").is_err());
    assert!(parse_checksum("").is_err());
    assert!(!is_trusted_source(
        "http://download.oracle.com/graalvm/21/latest/x.tar.gz"
    ));
    assert!(!is_trusted_source(
        "https://download.oracle.com.evil.example/x"
    ));
}
