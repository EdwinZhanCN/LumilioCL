use std::borrow::Cow;

use gpui::AssetSource;

use super::{Assets, FONTS, PIXEL_FONTS, PixelFace, register_fonts};

#[test]
fn every_declared_icon_path_loads() {
    let assets = Assets;
    for path in assets.list("icons/").expect("asset listing") {
        assert!(assets.load(&path).expect("asset load").is_some(), "{path}");
    }
}

#[test]
fn every_embedded_font_is_an_opentype_file() {
    for (index, data) in FONTS.iter().chain(&PIXEL_FONTS).enumerate() {
        let tag = &data[..4];
        assert!(
            tag == b"OTTO" || tag == [0, 1, 0, 0],
            "font {index} does not start with an sfnt header"
        );
    }
}

/// The Windows-platform strings of a font's `name` table with `id`.
fn names(font: &[u8], id: u16) -> Vec<String> {
    let u16_at = |at: usize| u16::from_be_bytes([font[at], font[at + 1]]);
    let u32_at = |at: usize| {
        u32::from_be_bytes([font[at], font[at + 1], font[at + 2], font[at + 3]]) as usize
    };
    let record = (0..usize::from(u16_at(4)))
        .map(|table| 12 + 16 * table)
        .find(|&record| &font[record..record + 4] == b"name")
        .expect("a name table");
    let table = u32_at(record + 8);
    let strings = table + usize::from(u16_at(table + 4));
    (0..usize::from(u16_at(table + 2)))
        .map(|name| table + 6 + 12 * name)
        .filter(|&name| u16_at(name) == 3 && u16_at(name + 6) == id)
        .map(|name| {
            let start = strings + usize::from(u16_at(name + 10));
            let units: Vec<u16> = font[start..start + usize::from(u16_at(name + 8))]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|unit| u16::from_be_bytes(*unit))
                .collect();
            String::from_utf16_lossy(&units)
        })
        .collect()
}

#[test]
fn the_pixel_faces_are_the_family_a_message_asks_for() {
    let styles: Vec<String> = PIXEL_FONTS
        .iter()
        .map(|font| {
            assert_eq!(names(font, 1), [crate::theme::PIXEL_FONT], "family");
            names(font, 2).concat()
        })
        .collect();
    assert_eq!(styles, ["Regular", "Bold", "Italic", "Bold-Italic"]);
}

#[gpui::test]
fn registering_the_fonts_makes_the_pixel_face_available(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        assert!(!cx.has_global::<PixelFace>(), "absent before registering");
        register_fonts(cx);
        assert!(cx.has_global::<PixelFace>());
    });
}

#[gpui::test]
fn the_text_system_accepts_every_embedded_font(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        let fonts = FONTS
            .iter()
            .chain(&PIXEL_FONTS)
            .map(|data| Cow::Borrowed(*data))
            .collect();
        cx.text_system()
            .add_fonts(fonts)
            .expect("every embedded font loads");
    });
}
