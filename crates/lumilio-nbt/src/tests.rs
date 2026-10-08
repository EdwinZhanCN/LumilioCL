pub(crate) mod build {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    pub fn name(out: &mut Vec<u8>, text: &str) {
        out.extend((text.len() as u16).to_be_bytes());
        out.extend(text.as_bytes());
    }

    pub fn string(out: &mut Vec<u8>, key: &str, value: &str) {
        out.push(8);
        name(out, key);
        name(out, value);
    }

    pub fn long(out: &mut Vec<u8>, key: &str, value: i64) {
        out.push(4);
        name(out, key);
        out.extend(value.to_be_bytes());
    }

    pub fn byte(out: &mut Vec<u8>, key: &str, value: i8) {
        out.push(1);
        name(out, key);
        out.push(value as u8);
    }

    pub fn compound_start(out: &mut Vec<u8>, key: &str) {
        out.push(10);
        name(out, key);
    }

    pub fn end(out: &mut Vec<u8>) {
        out.push(0);
    }

    pub fn gzip(data: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }
}

use super::*;
use build::*;

fn sample() -> Vec<u8> {
    let mut out = Vec::new();
    compound_start(&mut out, "");
    compound_start(&mut out, "Data");
    string(&mut out, "LevelName", "My World");
    long(&mut out, "LastPlayed", 1_700_000_000_000);
    byte(&mut out, "hardcore", 1);
    compound_start(&mut out, "Version");
    string(&mut out, "Name", "1.21.1");
    end(&mut out);
    end(&mut out);
    end(&mut out);
    out
}

#[test]
fn parses_nested_compounds() {
    let tag = parse(&sample()).unwrap();
    assert_eq!(
        tag.at(&["Data", "LevelName"]).unwrap().as_str(),
        Some("My World")
    );
    assert_eq!(
        tag.at(&["Data", "LastPlayed"]).unwrap().as_i64(),
        Some(1_700_000_000_000)
    );
    assert_eq!(tag.at(&["Data", "hardcore"]).unwrap().as_i64(), Some(1));
    assert_eq!(
        tag.at(&["Data", "Version", "Name"]).unwrap().as_str(),
        Some("1.21.1")
    );
    assert!(tag.at(&["Data", "missing"]).is_none());
}

#[test]
fn reads_gzip_and_plain_documents() {
    let plain = sample();
    assert_eq!(
        parse_maybe_gzip(&gzip(&plain)).unwrap(),
        parse(&plain).unwrap()
    );
    assert!(parse_maybe_gzip(&plain).is_ok());
}

#[test]
fn rejects_malformed_input_without_panicking() {
    let data = sample();
    for cut in 0..data.len() {
        assert!(parse(&data[..cut]).is_err(), "truncated at {cut}");
    }
    assert_eq!(parse(&[1, 0, 0]), Err(NbtError::NotACompound));
    // Unknown child tag id.
    let mut bad = Vec::new();
    compound_start(&mut bad, "");
    bad.push(99);
    name(&mut bad, "x");
    assert_eq!(parse(&bad), Err(NbtError::BadTag(99)));
}

#[test]
fn writing_then_reading_is_lossless() {
    let mut data = BTreeMap::new();
    data.insert("name".to_owned(), Tag::String("服务器 🎮 \0".to_owned()));
    data.insert("hidden".to_owned(), Tag::Byte(1));
    data.insert("big".to_owned(), Tag::Long(-5));
    data.insert("ints".to_owned(), Tag::IntArray(vec![1, -2]));
    let root = Tag::Compound(BTreeMap::from([(
        "servers".to_owned(),
        Tag::List(vec![Tag::Compound(data), Tag::Compound(BTreeMap::new())]),
    )]));
    assert_eq!(parse(&to_bytes(&root)).unwrap(), root);
    let empty = Tag::Compound(BTreeMap::from([("servers".to_owned(), Tag::List(vec![]))]));
    assert_eq!(parse(&to_bytes(&empty)).unwrap(), empty);
}

#[test]
fn supplementary_characters_use_the_games_encoding() {
    // U+1F3AE as a surrogate pair, three bytes each (not four bytes).
    assert_eq!(encode_modified_utf8("🎮").len(), 6);
    assert_eq!(decode_modified_utf8(&encode_modified_utf8("🎮")), "🎮");
    assert_eq!(encode_modified_utf8("\0"), [0xc0, 0x80]);
}

#[test]
fn deep_nesting_is_refused() {
    let mut out = Vec::new();
    compound_start(&mut out, "");
    for _ in 0..(MAX_DEPTH + 10) {
        compound_start(&mut out, "n");
    }
    assert_eq!(parse(&out), Err(NbtError::TooDeep));
}

#[test]
fn a_huge_declared_list_does_not_allocate_up_front() {
    let mut out = Vec::new();
    compound_start(&mut out, "");
    out.push(9);
    name(&mut out, "l");
    out.push(3);
    out.extend(i32::MAX.to_be_bytes());
    assert_eq!(parse(&out), Err(NbtError::Truncated));
}
#[test]
fn zlib_world_settings_are_parsed_with_the_same_bounds() {
    use std::io::Write;
    let tag = super::Tag::Compound(std::collections::BTreeMap::from([(
        "seed".into(),
        super::Tag::Long(-262),
    )]));
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&super::to_bytes(&tag)).unwrap();
    let compressed = encoder.finish().unwrap();
    assert_eq!(super::parse_maybe_gzip(&compressed).unwrap(), tag);
    assert!(super::parse_zlib(&compressed[..compressed.len() / 2]).is_err());
}
