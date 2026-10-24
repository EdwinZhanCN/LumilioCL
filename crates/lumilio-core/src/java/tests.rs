use super::*;
use lumilio_minecraft::{MachineArchitecture, PlatformFamily};

pub(super) fn install_as(
    root: &Path,
    name: &str,
    version: &str,
    vendor: &str,
    arch: &str,
) -> PathBuf {
    let home = root.join(name);
    fs::create_dir_all(home.join("bin")).unwrap();
    fs::write(home.join("bin/java"), "").unwrap();
    fs::write(
        home.join("release"),
        format!("IMPLEMENTOR=\"{vendor}\"\nJAVA_VERSION=\"{version}\"\nOS_ARCH=\"{arch}\"\n"),
    )
    .unwrap();
    home
}

fn install(root: &Path, name: &str, version: &str, vendor: &str) -> PathBuf {
    install_as(root, name, version, vendor, "aarch64")
}

#[test]
fn parses_legacy_and_modern_versions() {
    assert_eq!(parse_major("1.8.0_412"), Some(8));
    assert_eq!(parse_major("17"), Some(17));
    assert_eq!(parse_major("21.0.3"), Some(21));
    assert_eq!(parse_major("25-ea"), Some(25));
    assert_eq!(parse_major("1.7"), Some(7));
    assert_eq!(parse_major(""), None);
    assert_eq!(parse_major("abc"), None);
    assert_eq!(parse_major("99999999999999999999"), None);
}

#[test]
fn parses_release_files() {
    let props = parse_release("# c\n\nJAVA_VERSION=\"21.0.3\"\nBARE=yes\nnot a pair\n");
    assert_eq!(props["JAVA_VERSION"], "21.0.3");
    assert_eq!(props["BARE"], "yes");
    assert_eq!(props.len(), 2);
}

#[test]
fn normalizes_vendors() {
    assert_eq!(
        normalize_vendor("Oracle Corporation").as_deref(),
        Some("Oracle")
    );
    assert_eq!(normalize_vendor("N/A"), None);
    assert_eq!(
        normalize_vendor("Eclipse Adoptium").as_deref(),
        Some("Eclipse Adoptium")
    );
}

#[test]
fn discovers_installations_newest_first_without_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    install(dir.path(), "jdk-8", "1.8.0_412", "Azul Systems, Inc.");
    let home21 = install(dir.path(), "jdk-21", "21.0.3", "Eclipse Adoptium");
    install(dir.path(), "jdk-17", "17.0.11", "N/A");
    // A folder without a usable launcher is ignored.
    fs::create_dir_all(dir.path().join("not-java")).unwrap();
    let locator = JavaLocator::new([dir.path().to_owned(), home21.clone()]);
    let runtimes = locator.discover();
    let majors: Vec<_> = runtimes.iter().map(JavaRuntime::major).collect();
    assert_eq!(majors, [21, 17, 8]);
    assert_eq!(runtimes[0].vendor(), Some("Eclipse Adoptium"));
    assert_eq!(runtimes[1].vendor(), None);
    assert_eq!(runtimes[2].vendor(), Some("Azul"));
    assert_eq!(runtimes[0].architecture(), Some("aarch64"));
}

#[test]
fn finds_macos_bundles() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = dir.path().join("temurin-21.jdk/Contents");
    install(&bundle, "Home", "21.0.1", "Eclipse Adoptium");
    let runtimes = JavaLocator::new([dir.path().to_owned()]).discover();
    assert_eq!(runtimes.len(), 1);
    assert!(runtimes[0].home().ends_with("Contents/Home"));
}

#[test]
fn a_release_without_a_launcher_is_not_an_installation() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("broken");
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join("release"), "JAVA_VERSION=\"17\"").unwrap();
    assert!(JavaRuntime::inspect(&home).is_none());
}

#[test]
fn finds_the_official_launchers_runtimes() {
    let dir = tempfile::tempdir().unwrap();
    let tree = dir.path().join("runtime");
    // Linux and Windows: <component>/<platform>/<component>.
    install(
        &tree.join("java-runtime-delta/linux"),
        "java-runtime-delta",
        "21.0.7",
        "Microsoft",
    );
    // macOS: the same, inside jre.bundle.
    install(
        &tree.join("jre-legacy/mac-os/jre-legacy/jre.bundle/Contents"),
        "Home",
        "1.8.0_74",
        "Oracle Corporation",
    );
    // A component's manifest files beside it are not installations.
    fs::write(
        tree.join("java-runtime-delta/linux/java-runtime-delta.sha1"),
        "",
    )
    .unwrap();
    let runtimes = JavaLocator::default()
        .with_runtime_trees([tree, dir.path().join("missing")])
        .discover();
    let majors: Vec<_> = runtimes.iter().map(JavaRuntime::major).collect();
    assert_eq!(majors, [21, 8]);
    assert!(runtimes[1].home().ends_with("jre.bundle/Contents/Home"));
}

#[cfg(unix)]
#[test]
fn a_java_on_path_leads_to_its_real_home_once() {
    let dir = tempfile::tempdir().unwrap();
    let home = install(dir.path(), "jdk-17", "17.0.9", "BellSoft");
    // `/usr/bin/java` style: a link into the installation.
    let bin = dir.path().join("usr-bin");
    fs::create_dir_all(&bin).unwrap();
    std::os::unix::fs::symlink(home.join("bin/java"), bin.join("java")).unwrap();
    let path =
        std::env::join_paths([bin, dir.path().join("nothing-here"), home.join("bin")]).unwrap();
    let host = HostProfile::new(PlatformFamily::Linux, MachineArchitecture::X86_64, "");
    let runtimes = JavaLocator::default()
        .with_path_variable(&path, &host)
        .discover();
    assert_eq!(runtimes.len(), 1, "both PATH entries are the same Java");
    assert_eq!(
        fs::canonicalize(runtimes[0].home()).unwrap(),
        fs::canonicalize(&home).unwrap()
    );
}

#[test]
fn an_installation_is_found_from_its_executable_or_bundle() {
    let dir = tempfile::tempdir().unwrap();
    let home = install(dir.path(), "jdk", "21.0.1", "x");
    assert_eq!(
        JavaRuntime::at(&home.join("bin/java")).unwrap().home(),
        home
    );
    assert_eq!(JavaRuntime::at(&home).unwrap().home(), home);
    let bundle = dir.path().join("b.jdk");
    install(&bundle.join("Contents"), "Home", "17.0.1", "x");
    assert_eq!(JavaRuntime::at(&bundle).unwrap().major(), 17);
    assert!(JavaRuntime::at(&dir.path().join("none")).is_none());
}

#[test]
fn native_means_the_hosts_own_architecture() {
    let dir = tempfile::tempdir().unwrap();
    let arm = JavaRuntime::inspect(&install_as(dir.path(), "a", "21", "x", "aarch64")).unwrap();
    let intel = JavaRuntime::inspect(&install_as(dir.path(), "i", "21", "x", "x86_64")).unwrap();
    let amd = JavaRuntime::inspect(&install_as(dir.path(), "m", "21", "x", "amd64")).unwrap();
    let mac = HostProfile::new(PlatformFamily::MacOs, MachineArchitecture::Arm64, "");
    let linux = HostProfile::new(PlatformFamily::Linux, MachineArchitecture::X86_64, "");
    assert!(arm.is_native(&mac) && !intel.is_native(&mac));
    assert!(intel.is_native(&linux) && amd.is_native(&linux));
    assert!(arm.installed_under(dir.path()));
    assert!(!arm.installed_under(&dir.path().join("elsewhere")));
}

#[test]
fn only_apple_silicon_without_rosetta_lacks_it() {
    let dir = tempfile::tempdir().unwrap();
    let installed = dir.path().join("libRosettaRuntime");
    fs::write(&installed, "").unwrap();
    let missing = dir.path().join("none");
    let mac_arm = HostProfile::new(PlatformFamily::MacOs, MachineArchitecture::Arm64, "");
    let mac_intel = HostProfile::new(PlatformFamily::MacOs, MachineArchitecture::X86_64, "");
    let windows_arm = HostProfile::new(PlatformFamily::Windows, MachineArchitecture::Arm64, "");
    assert!(rosetta_missing_at(&mac_arm, &missing));
    assert!(!rosetta_missing_at(&mac_arm, &installed));
    assert!(!rosetta_missing_at(&mac_intel, &missing));
    assert!(!rosetta_missing_at(&windows_arm, &missing));
}

#[test]
fn without_os_arch_the_executable_header_tells_the_architecture() {
    let dir = tempfile::tempdir().unwrap();
    let with_launcher = |name: &str, launcher: &[u8]| {
        let home = dir.path().join(name);
        fs::create_dir_all(home.join("bin")).unwrap();
        fs::write(home.join("bin/java"), launcher).unwrap();
        fs::write(home.join("release"), "JAVA_VERSION=\"17.0.15\"\n").unwrap();
        JavaRuntime::inspect(&home).unwrap()
    };
    let mach_o = |cpu: u32| [&[0xcf, 0xfa, 0xed, 0xfe][..], &cpu.to_le_bytes()].concat();
    let elf = |machine: u16| {
        let mut header = vec![0x7f, b'E', b'L', b'F', 2, 1];
        header.resize(18, 0);
        header.extend(machine.to_le_bytes());
        header
    };
    let pe = |machine: u16| {
        let mut header = b"MZ".to_vec();
        header.resize(0x3c, 0);
        header.extend(0x80_u32.to_le_bytes());
        header.resize(0x80, 0);
        header.extend(b"PE\0\0");
        header.extend(machine.to_le_bytes());
        header
    };
    let universal = [0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 2];
    for (launcher, expected) in [
        (mach_o(0x0100_0007), Some("x86_64")),
        (mach_o(0x0100_000c), Some("aarch64")),
        (elf(0x3e), Some("x86_64")),
        (elf(0xb7), Some("aarch64")),
        (pe(0x8664), Some("x86_64")),
        (pe(0xaa64), Some("aarch64")),
        (pe(0x014c), Some("x86")),
        (universal.to_vec(), None),
        (b"#!/bin/sh\n".to_vec(), None),
        (Vec::new(), None),
    ] {
        let runtime = with_launcher(&format!("{expected:?}-{}", launcher.len()), &launcher);
        assert_eq!(runtime.architecture(), expected, "{launcher:02x?}");
    }
    // The release file still wins when it says.
    let home = install_as(dir.path(), "said", "21", "x", "x86_64");
    fs::write(home.join("bin/java"), mach_o(0x0100_000c)).unwrap();
    assert_eq!(
        JavaRuntime::inspect(&home).unwrap().architecture(),
        Some("x86_64")
    );
}
