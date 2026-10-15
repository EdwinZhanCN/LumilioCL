use std::collections::BTreeMap;
use std::io::Write;

use super::super::data_version;
use crate::nbt::{Tag, to_bytes};

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn compound(entries: Vec<(&str, Tag)>) -> Tag {
    Tag::Compound(
        entries
            .into_iter()
            .map(|(key, tag)| (key.to_owned(), tag))
            .collect::<BTreeMap<_, _>>(),
    )
}

#[test]
fn each_format_names_its_data_version() {
    let litematic = compound(vec![("MinecraftDataVersion", Tag::Int(3465))]);
    let sponge_v2 = compound(vec![("DataVersion", Tag::Int(2975))]);
    let sponge_v3 = compound(vec![(
        "Schematic",
        compound(vec![("DataVersion", Tag::Int(5023))]),
    )]);
    assert_eq!(data_version(&gzip(&to_bytes(&litematic))), Some(3465));
    assert_eq!(data_version(&gzip(&to_bytes(&sponge_v2))), Some(2975));
    assert_eq!(data_version(&to_bytes(&sponge_v3)), Some(5023));
    let mcedit = compound(vec![("Materials", Tag::String("Alpha".into()))]);
    assert_eq!(data_version(&gzip(&to_bytes(&mcedit))), None);
    assert_eq!(data_version(b"not a schematic"), None);
}
