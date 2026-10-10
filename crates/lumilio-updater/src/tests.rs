use semver::Version;

use crate::client::{UpdateError, UpdateRestriction, fallback_for_test};
use crate::manifest::{AssetTarget, find_asset};

const MAC: AssetTarget = AssetTarget {
    os: "macos",
    arch: "arm64",
    suffix: "-macos-arm64.dmg",
};

#[test]
fn finds_the_current_platform_package_and_parses_semver() {
    let manifest = format!(
        "{}  LumilioCL-0.2.0-beta.1-macos-arm64.dmg\n{}  LumilioCL-0.2.0-beta.1-windows-x64-setup.exe\n",
        "a".repeat(64),
        "b".repeat(64)
    );
    let found = find_asset(&manifest, &MAC).unwrap();
    assert_eq!(found.version(), &Version::parse("0.2.0-beta.1").unwrap());
    assert_eq!(found.file_name(), "LumilioCL-0.2.0-beta.1-macos-arm64.dmg");
}

#[test]
fn rejects_missing_duplicate_and_invalid_target_assets() {
    assert!(find_asset("", &MAC).is_err());
    let manifest = format!(
        "{}  LumilioCL-0.2.0-macos-arm64.dmg\n{}  LumilioCL-0.3.0-macos-arm64.dmg\n",
        "a".repeat(64),
        "b".repeat(64)
    );
    assert!(find_asset(&manifest, &MAC).is_err());
    assert!(
        find_asset(
            &format!("{}  LumilioCL-latest-macos-arm64.dmg\n", "c".repeat(64)),
            &MAC
        )
        .is_err()
    );
    assert!(
        find_asset(
            &format!("{}  LumilioCL-0.2.0-macos-arm64.dmg\n", "not-a-hash"),
            &MAC
        )
        .is_err()
    );
}

#[test]
fn versions_follow_semver_ordering() {
    assert!(Version::parse("0.2.0").unwrap() > Version::parse("0.1.9").unwrap());
    assert!(Version::parse("0.2.0-beta.2").unwrap() > Version::parse("0.2.0-beta.1").unwrap());
    assert!(Version::parse("0.2.0").unwrap() > Version::parse("0.2.0-beta.1").unwrap());
}

#[tokio::test]
async fn a_failed_worker_request_falls_back_to_github() {
    let fetched = fallback_for_test::<&str>(
        Err(UpdateError::Request("worker unavailable".to_owned())),
        async { Ok("github manifest") },
    )
    .await
    .unwrap();
    assert_eq!(fetched, "github manifest");
}

#[test]
fn build_restrictions_have_stable_values() {
    assert_eq!(
        UpdateRestriction::from_build_value("portable-windows"),
        UpdateRestriction::PortableWindows
    );
    assert_eq!(
        UpdateRestriction::from_build_value("package-managed-linux"),
        UpdateRestriction::PackageManagedLinux
    );
}
