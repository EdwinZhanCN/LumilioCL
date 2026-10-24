#[test]
fn an_explicitly_empty_address_marks_a_generated_file() {
    let host = crate::environment::HostProfile::current();
    let generated: LibraryDependency = serde_json::from_str(
        r#"{"name":"net.minecraftforge:forge:26.3-66.0.9:client",
                "downloads":{"artifact":{"path":"net/minecraftforge/forge/26.3-66.0.9/forge-26.3-66.0.9-client.jar",
                "url":"","sha1":"6254fd332277905b955a8502228e23a7eae396a8","size":1}}}"#,
    )
    .unwrap();
    let file = generated.resolve_file(&host);
    assert!(file.generated, "made by the installer, never downloaded");
    assert_eq!(file.download.url(), None);
    // Without any address the library repository is still the fallback.
    let plain: LibraryDependency = serde_json::from_str(r#"{"name":"a:b:1"}"#).unwrap();
    let file = plain.resolve_file(&host);
    assert!(!file.generated);
    assert_eq!(
        file.download.url(),
        Some("https://libraries.minecraft.net/a/b/1/b-1.jar")
    );
}

use super::{ArgumentTemplate, LibraryDependency, ReleaseManifest, ReleaseSet};

#[test]
fn unknown_members_do_not_prevent_decoding() {
    let release =
        ReleaseManifest::decode_json(r#"{"id":"release","custom":{"preserved":true}}"#).unwrap();

    assert_eq!(release.id(), "release");
}

#[test]
fn inheritance_cycles_are_rejected() {
    let first = ReleaseManifest::decode_json(r#"{"id":"first","inheritsFrom":"second"}"#).unwrap();
    let second = ReleaseManifest::decode_json(r#"{"id":"second","inheritsFrom":"first"}"#).unwrap();
    let mut releases = ReleaseSet::new();
    releases.insert(first);
    releases.insert(second);

    assert!(releases.resolve("first").is_err());
}

#[test]
fn argument_string_decodes_as_a_literal() {
    let argument: ArgumentTemplate = serde_json::from_str(r#""--demo""#).unwrap();

    assert_eq!(argument, ArgumentTemplate::Literal("--demo".to_owned()));
}
