use super::*;
use crate::java::JavaLocator;
use crate::java::tests::install_as;
use lumilio_minecraft::{MachineArchitecture, PlatformFamily};

fn mac() -> HostProfile {
    HostProfile::new(PlatformFamily::MacOs, MachineArchitecture::Arm64, "")
}

fn linux() -> HostProfile {
    HostProfile::new(PlatformFamily::Linux, MachineArchitecture::X86_64, "")
}

/// Installs `(folder, version, arch)` runtimes and discovers them.
fn runtimes(dir: &Path, which: &[(&str, &str, &str)]) -> Vec<JavaRuntime> {
    for (name, version, arch) in which {
        install_as(dir, name, version, "x", arch);
    }
    JavaLocator::new([dir.to_owned()]).discover()
}

fn picked(runtimes: &[JavaRuntime], needs: &JavaNeeds, host: &HostProfile) -> Option<u32> {
    choose(runtimes, needs, None, host).map(|choice| choice.runtime.major())
}

#[test]
fn chooses_by_requirement_then_preference() {
    let dir = tempfile::tempdir().unwrap();
    let runtimes = runtimes(
        dir.path(),
        &[
            ("a", "1.8.0_1", "aarch64"),
            ("b", "17.0.1", "aarch64"),
            ("c", "21.0.1", "aarch64"),
        ],
    );
    let vanilla = |required| JavaNeeds::new("1.21", Loader::Vanilla, required);
    assert_eq!(picked(&runtimes, &vanilla(Some(17)), &mac()), Some(17));
    // No exact match: the lowest newer major.
    assert_eq!(picked(&runtimes, &vanilla(Some(18)), &mac()), Some(21));
    // Nothing new enough.
    assert_eq!(picked(&runtimes, &vanilla(Some(25)), &mac()), None);
    // No requirement: Java 8 when present.
    assert_eq!(picked(&runtimes, &vanilla(None), &mac()), Some(8));
    // An explicit choice wins over the requirement, and says what it breaks.
    let java8 = runtimes.iter().find(|r| r.major() == 8).unwrap();
    let choice = choose(
        &runtimes,
        &vanilla(Some(21)),
        Some(java8.executable()),
        &mac(),
    )
    .unwrap();
    assert_eq!(choice.runtime.major(), 8);
    assert!(choice.chosen_by_user && choice.breaks_required());
    assert_eq!(choice.advised_major(), Some(21));
    // An unknown preference falls back to the requirement.
    assert_eq!(
        choose(
            &runtimes,
            &vanilla(Some(21)),
            Some(Path::new("/nowhere/java")),
            &mac()
        )
        .unwrap()
        .runtime
        .major(),
        21
    );
}

#[test]
fn a_preferred_java_outside_the_search_folders_is_still_used() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = install_as(dir.path(), "custom", "21.0.2", "x", "aarch64");
    let needs = JavaNeeds::new("1.21", Loader::Vanilla, Some(21));
    let choice = choose(&[], &needs, Some(&elsewhere.join("bin/java")), &mac()).unwrap();
    assert_eq!(choice.runtime.home(), elsewhere);
    assert!(choice.broken.is_empty());
}

#[test]
fn old_forge_through_launchwrapper_never_gets_a_newer_java() {
    let dir = tempfile::tempdir().unwrap();
    let both = runtimes(
        dir.path(),
        &[("j8", "1.8.0_412", "aarch64"), ("j21", "21.0.3", "aarch64")],
    );
    let forge = JavaNeeds::new("1.12.2", Loader::Forge, Some(8)).with_start(
        "net.minecraft.launchwrapper.Launch",
        &["/libraries/net/minecraft/launchwrapper/1.12/launchwrapper-1.12.jar"],
    );
    assert!(forge.launch_wrapper);
    assert_eq!(picked(&both, &forge, &mac()), Some(8));

    let only21: Vec<_> = both.iter().filter(|r| r.major() == 21).cloned().collect();
    assert_eq!(picked(&only21, &forge, &mac()), None, "Java 21 would crash");
    assert_eq!(install_major(&forge, &mac()), Some(8));

    // A newer LaunchWrapper works on newer Java.
    let patched = JavaNeeds::new("1.12.2", Loader::Forge, Some(8)).with_start(
        "net.minecraft.launchwrapper.Launch",
        &["net.minecraft:launchwrapper:1.13"],
    );
    assert!(!patched.launch_wrapper);
}

#[test]
fn modded_versions_prefer_their_tested_java_but_still_start_on_another() {
    let dir = tempfile::tempdir().unwrap();
    let all = runtimes(
        dir.path(),
        &[
            ("j8", "1.8.0_412", "aarch64"),
            ("j17", "17.0.9", "aarch64"),
            ("j21", "21.0.3", "aarch64"),
        ],
    );
    let forge = |version: &str, required| JavaNeeds::new(version, Loader::Forge, required);
    assert_eq!(picked(&all, &forge("1.16.5", Some(8)), &mac()), Some(8));
    assert_eq!(picked(&all, &forge("1.20.1", Some(17)), &mac()), Some(17));
    assert_eq!(picked(&all, &forge("1.20.6", Some(21)), &mac()), Some(21));
    // Vanilla 1.16.5 has no such rule: the lowest from its minimum.
    assert_eq!(
        picked(
            &all,
            &JavaNeeds::new("1.16.5", Loader::Vanilla, Some(8)),
            &mac()
        ),
        Some(8)
    );

    // Without the tested Java the game still starts, and says what is advised.
    let no8: Vec<_> = all.iter().filter(|r| r.major() != 8).cloned().collect();
    let choice = choose(&no8, &forge("1.16.5", Some(8)), None, &mac()).unwrap();
    assert_eq!(choice.runtime.major(), 17);
    assert!(!choice.breaks_required());
    assert_eq!(choice.advised_major(), Some(8));
}

#[test]
fn a_cap_below_the_release_minimum_does_not_apply() {
    // A later release of a line may ask for more than Forge's table says.
    let needs = JavaNeeds::new("26.1", Loader::NeoForge, Some(25));
    assert_eq!(rules(&needs, &mac()), [JavaRule::Minimum(25)]);
    let needs = JavaNeeds::new("1.20.6", Loader::NeoForge, Some(21));
    assert_eq!(
        rules(&needs, &mac()),
        [JavaRule::Minimum(21), JavaRule::ModLoader(21)]
    );
    // Snapshots are not compared with releases.
    assert_eq!(
        rules(&JavaNeeds::new("24w14a", Loader::Vanilla, Some(21)), &mac()),
        [JavaRule::Minimum(21)]
    );
}

#[test]
fn old_versions_on_64_bit_linux_need_java_8() {
    let dir = tempfile::tempdir().unwrap();
    let both = runtimes(
        dir.path(),
        &[("j8", "1.8.0_412", "amd64"), ("j17", "17.0.9", "amd64")],
    );
    let old = JavaNeeds::new("1.12.2", Loader::Vanilla, Some(8));
    assert_eq!(picked(&both, &old, &linux()), Some(8));
    assert!(rules(&old, &linux()).contains(&JavaRule::LegacyLinux));
    assert!(!rules(&old, &mac()).contains(&JavaRule::LegacyLinux));
    let beta = JavaNeeds::new("b1.7.3", Loader::Vanilla, None);
    assert_eq!(install_major(&beta, &linux()), Some(8));
    assert_eq!(
        install_major(&JavaNeeds::new("1.21", Loader::Vanilla, Some(21)), &linux()),
        Some(21)
    );
}

#[test]
fn a_native_java_comes_before_a_translated_one() {
    let dir = tempfile::tempdir().unwrap();
    let both = runtimes(
        dir.path(),
        &[
            ("a-intel", "21.0.3", "x86_64"),
            ("b-arm", "21.0.1", "aarch64"),
        ],
    );
    let needs = JavaNeeds::new("1.21", Loader::Vanilla, Some(21));
    let choice = choose(&both, &needs, None, &mac()).unwrap();
    assert_eq!(choice.runtime.architecture(), Some("aarch64"));
    // The translated one is still a fallback.
    let intel: Vec<_> = both
        .iter()
        .filter(|r| r.architecture() == Some("x86_64"))
        .cloned()
        .collect();
    assert_eq!(picked(&intel, &needs, &mac()), Some(21));
}

#[test]
fn intel_only_natives_take_an_intel_java_on_arm() {
    let dir = tempfile::tempdir().unwrap();
    let all = runtimes(
        dir.path(),
        &[
            ("arm8", "1.8.0_412", "aarch64"),
            ("arm17", "17.0.15", "aarch64"),
            ("arm21", "21.0.7", "aarch64"),
            ("intel8", "1.8.0_74", "x86_64"),
            ("intel17", "17.0.15", "x86_64"),
        ],
    );
    let windows_arm = HostProfile::new(PlatformFamily::Windows, MachineArchitecture::Arm64, "");
    let intel_choice = |needs: &JavaNeeds, host: &HostProfile| {
        choose(&all, needs, None, host).map(|choice| {
            (
                choice.runtime.major(),
                choice.runtime.architecture().map(str::to_owned),
            )
        })
    };
    let intel = |major| Some((major, Some("x86_64".to_owned())));
    let arm = |major| Some((major, Some("aarch64".to_owned())));

    // 1.6.4: LWJGL 2 natives as an archive (no requirement in its manifest).
    let lwjgl2 = JavaNeeds::new("1.6.4", Loader::Vanilla, None).with_natives([
        "/libraries/org/lwjgl/lwjgl/lwjgl/2.9.0/lwjgl-2.9.0.jar",
        "/libraries/org/lwjgl/lwjgl/lwjgl-platform/2.9.0/lwjgl-platform-2.9.0-natives-osx.jar",
    ]);
    assert_eq!(lwjgl2.natives, ["osx"]);
    assert_eq!(intel_choice(&lwjgl2, &mac()), intel(8));
    assert_eq!(rules(&lwjgl2, &mac()), [JavaRule::IntelNatives(8)]);
    // 1.18.2: LWJGL 3 natives on the class path.
    let lwjgl3 = JavaNeeds::new("1.18.2", Loader::Vanilla, Some(17)).with_natives([
        "org.lwjgl:lwjgl:3.2.2",
        "org.lwjgl:lwjgl:3.2.2:natives-macos",
        "org.lwjgl:lwjgl:3.2.2:natives-windows",
    ]);
    assert_eq!(intel_choice(&lwjgl3, &mac()), intel(17));
    assert_eq!(intel_choice(&lwjgl3, &windows_arm), intel(17));
    // 1.19 ships Arm natives: the native Java as before.
    let arm_natives = JavaNeeds::new("1.19", Loader::Vanilla, Some(17)).with_natives([
        "org.lwjgl:lwjgl:3.3.1:natives-macos",
        "org.lwjgl:lwjgl:3.3.1:natives-macos-arm64",
        "org.lwjgl:lwjgl:3.3.1:natives-windows",
        "org.lwjgl:lwjgl:3.3.1:natives-windows-arm64",
    ]);
    assert_eq!(intel_choice(&arm_natives, &mac()), arm(17));
    assert_eq!(intel_choice(&arm_natives, &windows_arm), arm(17));
    // Only Arm hosts that translate Intel code are concerned.
    let intel_rule = |needs: &JavaNeeds, host: &HostProfile| {
        rules(needs, host)
            .iter()
            .any(|rule| matches!(rule, JavaRule::IntelNatives(_)))
    };
    let linux_arm = HostProfile::new(PlatformFamily::Linux, MachineArchitecture::Arm64, "");
    assert!(!intel_rule(&lwjgl2, &linux()));
    assert!(!intel_rule(&lwjgl3, &linux_arm));
    // An Arm Java chosen by hand breaks it.
    let arm8 = all.iter().find(|r| r.home().ends_with("arm8")).unwrap();
    let choice = choose(&all, &lwjgl2, Some(arm8.executable()), &mac()).unwrap();
    assert!(choice.breaks_required());
    assert_eq!(choice.advised_major(), Some(8));

    // Nothing Intel installed: download Intel Java 8, or the release's minimum.
    let arm_only: Vec<_> = all
        .iter()
        .filter(|r| r.architecture() == Some("aarch64"))
        .cloned()
        .collect();
    assert_eq!(choose(&arm_only, &lwjgl2, None, &mac()), None);
    assert_eq!(install_major(&lwjgl2, &mac()), Some(8));
    assert_eq!(install_build(&lwjgl2, &mac()), JavaBuild::Intel);
    assert_eq!(install_major(&lwjgl3, &mac()), Some(17));
    assert_eq!(install_build(&arm_natives, &mac()), JavaBuild::Any);
    assert_eq!(install_build(&lwjgl2, &linux()), JavaBuild::Any);
}

#[test]
fn a_release_names_its_natives_by_classifier_and_by_system() {
    let release = lumilio_minecraft::ReleaseManifest::decode_json(
        r#"{
            "id": "1.6.4",
            "mainClass": "net.minecraft.client.main.Main",
            "libraries": [
                {"name": "org.lwjgl.lwjgl:lwjgl:2.9.0"},
                {
                    "name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.0",
                    "natives": {"linux": "natives-linux", "osx": "natives-osx", "windows": "natives-windows"}
                },
                {"name": "org.lwjgl:lwjgl:3.3.1:natives-macos-arm64"}
            ]
        }"#,
    )
    .unwrap();
    let record: InstanceRecord =
        serde_json::from_str(r#"{"id": "t", "name": "t", "game_version": "1.6.4"}"#).unwrap();
    let needs = JavaNeeds::of_release(&record, &release);
    assert_eq!(needs.natives, ["macos-arm64", "linux", "osx", "windows"]);
}

#[test]
fn the_newest_build_of_a_major_wins() {
    let dir = tempfile::tempdir().unwrap();
    let both = runtimes(
        dir.path(),
        &[("a", "21.0.1", "aarch64"), ("b", "21.0.10", "aarch64")],
    );
    let needs = JavaNeeds::new("1.21", Loader::Vanilla, Some(21));
    let choice = choose(&both, &needs, None, &mac()).unwrap();
    assert_eq!(choice.runtime.version(), "21.0.10");
}
