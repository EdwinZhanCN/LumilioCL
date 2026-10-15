use super::{baked, jar_1_13_2, jar_26_3};
use crate::{BakedPack, BlockCatalog, Bundle, ContainerError, write_bundle};

#[test]
fn a_pack_reads_back_equal_and_writes_the_same_bytes() {
    let pack = baked(jar_26_3());
    assert!(!pack.assets.parents.is_empty(), "templates are written too");
    let bytes = pack.to_bytes().unwrap();
    let back = BakedPack::from_bytes(&bytes).unwrap();
    assert_eq!(back, pack);
    assert_eq!(back.to_bytes().unwrap(), bytes);
}

/// Versions share their pixels in a bundle; each pack reads back whole.
#[test]
fn a_bundle_stores_shared_images_once() {
    let old = baked(jar_1_13_2());
    let new = baked(jar_26_3());
    let bytes = write_bundle([("26.3", &new), ("1.13.2", &old)]).unwrap();
    let bundle = Bundle::read(&bytes).unwrap();

    assert_eq!(bundle.keys().collect::<Vec<_>>(), ["1.13.2", "26.3"]);
    assert_eq!(bundle.image_count(), 4);
    assert_eq!(bundle.pack("1.13.2").unwrap(), old);
    assert_eq!(bundle.pack("26.3").unwrap(), new);
    assert_eq!(
        bundle.pack("1.20.1").unwrap_err(),
        ContainerError::MissingPack("1.20.1".into())
    );
}

/// Damage anywhere gives an error that says what is wrong, never a panic.
#[test]
fn a_damaged_file_is_a_readable_error() {
    let bytes = baked(jar_26_3()).to_bytes().unwrap();
    let container = zstd::decode_all(bytes.as_slice()).unwrap();
    let pack = |container: &[u8]| -> Result<BakedPack, ContainerError> {
        BakedPack::from_bytes(&zstd::encode_all(container, 1).unwrap())
    };
    let edited = |edit: &dyn Fn(&mut Vec<u8>)| {
        let mut copy = container.clone();
        edit(&mut copy);
        pack(&copy)
    };

    let cases = [
        ("empty", BakedPack::from_bytes(&[])),
        ("not zstd", BakedPack::from_bytes(b"plain text")),
        (
            "cut short",
            BakedPack::from_bytes(&bytes[..bytes.len() / 2]),
        ),
        ("wrong magic", edited(&|c| c[0] = b'X')),
        ("newer format", edited(&|c| c[8] = 2)),
        (
            "index too long",
            edited(&|c| c[12..16].copy_from_slice(&u32::MAX.to_le_bytes())),
        ),
        ("index garbled", edited(&|c| c[16] = b'#')),
        ("data cut short", edited(&|c| c.truncate(c.len() - 1))),
        ("pixel changed", edited(&|c| *c.last_mut().unwrap() ^= 0xff)),
    ];
    for (name, result) in cases {
        let error = result.expect_err(name);
        let message = error.to_string();
        assert!(message.contains("baked block-asset"), "{name}: {message}");
    }
    assert_eq!(
        edited(&|c| c[8] = 2).unwrap_err(),
        ContainerError::UnsupportedFormat(2)
    );
}

/// A bundle's catalog names each block with the property values its variant
/// keys list, and multipart properties as open, without decoding images.
#[test]
fn a_bundle_catalog_lists_blocks_and_their_property_values() {
    let old = baked(jar_1_13_2());
    let bytes = write_bundle([("1.13.2", &old)]).unwrap();
    let catalog = Bundle::read(&bytes).unwrap().catalog("1.13.2").unwrap();

    assert_eq!(catalog, BlockCatalog::of(&old.assets));
    assert!(catalog.has_block("minecraft:lamp"));
    assert!(!catalog.has_block("minecraft:cherry_log"));
    let facing = catalog.property("minecraft:lamp", "facing").unwrap();
    assert!(facing.allows("east") && !facing.allows("south"));
    assert!(catalog.property("minecraft:lamp", "waterlogged").is_none());
    let north = catalog.property("minecraft:lamp_wire", "north").unwrap();
    assert!(north.allows("anything"), "multipart values are open");
    assert!(catalog.property("minecraft:lamp_wire", "power").is_some());
}
