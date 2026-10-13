use super::{of_bytes, of_file};

// Expected values from a reference implementation that reproduces the
// fingerprint CurseForge publishes for jei-1.20.1-forge-15.62.0.219.jar
// (4164405417).
#[test]
fn matches_curseforge_for_every_tail_length() {
    for (text, expected) in [
        ("", 1_540_447_798),
        ("a", 626_045_324),
        ("ab", 1_692_487_918),
        ("abc", 1_621_425_345),
        ("abcd", 3_376_380_438),
        ("Hello, world!", 2_173_663_876),
        ("Minecraft", 2_572_622_835),
    ] {
        assert_eq!(of_bytes(text.as_bytes()), expected, "{text:?}");
    }
}

#[test]
fn whitespace_bytes_do_not_count() {
    assert_eq!(of_bytes(b" a b\nc\td \r"), of_bytes(b"abcd"));
    assert_ne!(
        of_bytes(b"a\x0bbcd"),
        of_bytes(b"abcd"),
        "only the four whitespace bytes"
    );
}

#[test]
fn a_file_hashes_like_its_bytes_across_buffer_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.jar");
    let bytes: Vec<u8> = (0..200_003_u32)
        .map(|at| b"x y\nz"[(at % 5) as usize] ^ u8::try_from(at % 3).unwrap())
        .collect();
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(of_file(&path).unwrap(), of_bytes(&bytes));
    assert!(of_file(&dir.path().join("missing")).is_err());
}
