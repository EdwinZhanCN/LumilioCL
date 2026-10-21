//! Embedded SVG assets approved by ADR 0003.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};
use gpui_component::IconNamed;

#[derive(Clone, Copy, Debug, Default)]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let data = match path {
            "icons/phosphor/house.svg" => {
                include_bytes!("../assets/icons/phosphor/house.svg").as_slice()
            }
            "icons/phosphor/books.svg" => {
                include_bytes!("../assets/icons/phosphor/books.svg").as_slice()
            }
            "icons/phosphor/compass.svg" => {
                include_bytes!("../assets/icons/phosphor/compass.svg").as_slice()
            }
            "icons/phosphor/pulse.svg" => {
                include_bytes!("../assets/icons/phosphor/pulse.svg").as_slice()
            }
            "icons/phosphor/user.svg" => {
                include_bytes!("../assets/icons/phosphor/user.svg").as_slice()
            }
            "icons/lucide/settings.svg" => {
                include_bytes!("../assets/icons/lucide/settings.svg").as_slice()
            }
            "icons/lucide/copy.svg" => include_bytes!("../assets/icons/lucide/copy.svg").as_slice(),
            "icons/lucide/download.svg" => {
                include_bytes!("../assets/icons/lucide/download.svg").as_slice()
            }
            "icons/lucide/plus.svg" => include_bytes!("../assets/icons/lucide/plus.svg").as_slice(),
            "icons/lucide/play.svg" => include_bytes!("../assets/icons/lucide/play.svg").as_slice(),
            "icons/lucide/refresh-cw.svg" => {
                include_bytes!("../assets/icons/lucide/refresh-cw.svg").as_slice()
            }
            "icons/lucide/info.svg" => include_bytes!("../assets/icons/lucide/info.svg").as_slice(),
            "icons/lucide/languages.svg" => {
                include_bytes!("../assets/icons/lucide/languages.svg").as_slice()
            }
            "icons/lucide/arrow-right.svg" => {
                include_bytes!("../assets/icons/lucide/arrow-right.svg").as_slice()
            }
            "icons/lucide/x.svg" => include_bytes!("../assets/icons/lucide/x.svg").as_slice(),
            "icons/lucide/square.svg" => {
                include_bytes!("../assets/icons/lucide/square.svg").as_slice()
            }
            "icons/lucide/star.svg" => include_bytes!("../assets/icons/lucide/star.svg").as_slice(),
            "icons/lucide/star-filled.svg" => {
                include_bytes!("../assets/icons/lucide/star-filled.svg").as_slice()
            }
            "icons/lucide/chevron-left.svg" => {
                include_bytes!("../assets/icons/lucide/chevron-left.svg").as_slice()
            }
            "icons/lucide/search.svg" => {
                include_bytes!("../assets/icons/lucide/search.svg").as_slice()
            }
            "icons/lucide/heart.svg" => {
                include_bytes!("../assets/icons/lucide/heart.svg").as_slice()
            }
            "icons/lucide/clock.svg" => {
                include_bytes!("../assets/icons/lucide/clock.svg").as_slice()
            }
            "icons/lucide/chevron-right.svg" => {
                include_bytes!("../assets/icons/lucide/chevron-right.svg").as_slice()
            }
            "icons/lucide/external-link.svg" => {
                include_bytes!("../assets/icons/lucide/external-link.svg").as_slice()
            }
            "icons/lucide/ellipsis.svg" => {
                include_bytes!("../assets/icons/lucide/ellipsis.svg").as_slice()
            }
            "icons/lucide/trash-2.svg" => {
                include_bytes!("../assets/icons/lucide/trash-2.svg").as_slice()
            }
            "icons/lucide/chevron-down.svg" => {
                include_bytes!("../assets/icons/lucide/chevron-down.svg").as_slice()
            }
            "icons/lucide/chevrons-up-down.svg" => {
                include_bytes!("../assets/icons/lucide/chevrons-up-down.svg").as_slice()
            }
            "icons/lucide/ban.svg" => include_bytes!("../assets/icons/lucide/ban.svg").as_slice(),
            "icons/lucide/lock.svg" => include_bytes!("../assets/icons/lucide/lock.svg").as_slice(),
            "icons/lucide/chevron-up.svg" => {
                include_bytes!("../assets/icons/lucide/chevron-up.svg").as_slice()
            }
            "icons/lucide/maximize.svg" => {
                include_bytes!("../assets/icons/lucide/maximize.svg").as_slice()
            }
            _ => return Ok(None),
        };
        Ok(Some(Cow::Borrowed(data)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        const ASSETS: [&str; 32] = [
            "icons/phosphor/house.svg",
            "icons/phosphor/books.svg",
            "icons/phosphor/compass.svg",
            "icons/phosphor/pulse.svg",
            "icons/lucide/download.svg",
            "icons/lucide/plus.svg",
            "icons/lucide/play.svg",
            "icons/lucide/refresh-cw.svg",
            "icons/lucide/info.svg",
            "icons/lucide/languages.svg",
            "icons/lucide/arrow-right.svg",
            "icons/lucide/x.svg",
            "icons/lucide/square.svg",
            "icons/lucide/star.svg",
            "icons/lucide/star-filled.svg",
            "icons/lucide/chevron-left.svg",
            "icons/lucide/search.svg",
            "icons/lucide/heart.svg",
            "icons/lucide/clock.svg",
            "icons/lucide/chevron-right.svg",
            "icons/lucide/external-link.svg",
            "icons/lucide/ellipsis.svg",
            "icons/lucide/chevrons-up-down.svg",
            "icons/lucide/chevron-down.svg",
            "icons/lucide/trash-2.svg",
            "icons/phosphor/user.svg",
            "icons/lucide/copy.svg",
            "icons/lucide/settings.svg",
            "icons/lucide/ban.svg",
            "icons/lucide/lock.svg",
            "icons/lucide/chevron-up.svg",
            "icons/lucide/maximize.svg",
        ];

        Ok(ASSETS
            .into_iter()
            .filter(|asset| asset.starts_with(path))
            .map(SharedString::from)
            .collect())
    }
}

/// The brand typefaces (design language §13), embedded so registering them
/// reads no file. All three are SIL OFL; see `assets/ATTRIBUTIONS.md`.
const FONTS: [&[u8]; 7] = [
    include_bytes!("../assets/fonts/SpaceGrotesk-Light.otf"),
    include_bytes!("../assets/fonts/SpaceGrotesk-Regular.otf"),
    include_bytes!("../assets/fonts/SpaceGrotesk-Medium.otf"),
    include_bytes!("../assets/fonts/SpaceGrotesk-Bold.otf"),
    include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMono-Medium.ttf"),
    include_bytes!("../assets/fonts/DSEG7Classic-Regular.ttf"),
];

/// The pixel face a server's message is drawn in (design language §13):
/// Monocraft, SIL OFL, made from its own pixel tables, not the game's files.
const PIXEL_FONTS: [&[u8]; 4] = [
    include_bytes!("../assets/fonts/Monocraft-Regular.otf"),
    include_bytes!("../assets/fonts/Monocraft-Bold.otf"),
    include_bytes!("../assets/fonts/Monocraft-Italic.otf"),
    include_bytes!("../assets/fonts/Monocraft-Bold-Italic.otf"),
];

/// Present once the pixel face is registered. Without it a message keeps
/// the interface face and its colours.
pub struct PixelFace;

impl gpui::Global for PixelFace {}

/// Registers the brand typefaces with the text system, from memory. Chinese
/// is not bundled: it resolves through the platform's own fallback (ADR 0013).
/// The pixel face is registered on its own, so losing it loses nothing else.
pub fn register_fonts(cx: &mut gpui::App) {
    let fonts = FONTS.iter().map(|data| Cow::Borrowed(*data)).collect();
    if let Err(reason) = cx.text_system().add_fonts(fonts) {
        log::error!("could not register its typefaces: {reason}");
    }
    let pixel = PIXEL_FONTS
        .iter()
        .map(|data| Cow::Borrowed(*data))
        .collect();
    match cx.text_system().add_fonts(pixel) {
        Ok(()) => cx.set_global(PixelFace),
        Err(reason) => log::error!("could not register its pixel face: {reason}"),
    }
}

/// Small interface glyphs used by the page kit.
#[derive(Clone, Copy)]
pub enum UiIcon {
    Star,
    StarFilled,
    Back,
    Search,
    Play,
    Download,
    Refresh,
    Info,
    Plus,
    Close,
    Heart,
    Clock,
    Next,
    External,
    More,
    Switch,
    Expand,
    Trash,
    Copy,
    Ban,
    Lock,
    Collapse,
    Maximize,
    Languages,
}

impl IconNamed for UiIcon {
    fn path(self) -> SharedString {
        match self {
            Self::Star => "icons/lucide/star.svg",
            Self::StarFilled => "icons/lucide/star-filled.svg",
            Self::Back => "icons/lucide/chevron-left.svg",
            Self::Search => "icons/lucide/search.svg",
            Self::Play => "icons/lucide/play.svg",
            Self::Download => "icons/lucide/download.svg",
            Self::Refresh => "icons/lucide/refresh-cw.svg",
            Self::Info => "icons/lucide/info.svg",
            Self::Plus => "icons/lucide/plus.svg",
            Self::Close => "icons/lucide/x.svg",
            Self::Heart => "icons/lucide/heart.svg",
            Self::Clock => "icons/lucide/clock.svg",
            Self::Next => "icons/lucide/chevron-right.svg",
            Self::External => "icons/lucide/external-link.svg",
            Self::More => "icons/lucide/ellipsis.svg",
            Self::Switch => "icons/lucide/chevrons-up-down.svg",
            Self::Expand => "icons/lucide/chevron-down.svg",
            Self::Trash => "icons/lucide/trash-2.svg",
            Self::Copy => "icons/lucide/copy.svg",
            Self::Ban => "icons/lucide/ban.svg",
            Self::Lock => "icons/lucide/lock.svg",
            Self::Collapse => "icons/lucide/chevron-up.svg",
            Self::Maximize => "icons/lucide/maximize.svg",
            Self::Languages => "icons/lucide/languages.svg",
        }
        .into()
    }
}

#[derive(Clone, Copy)]
pub enum LandmarkIcon {
    Home,
    Library,
    Discover,
    Activity,
    Accounts,
    Settings,
}

impl IconNamed for LandmarkIcon {
    fn path(self) -> SharedString {
        match self {
            Self::Home => "icons/phosphor/house.svg",
            Self::Library => "icons/phosphor/books.svg",
            Self::Discover => "icons/phosphor/compass.svg",
            Self::Activity => "icons/phosphor/pulse.svg",
            Self::Accounts => "icons/phosphor/user.svg",
            Self::Settings => "icons/lucide/settings.svg",
        }
        .into()
    }
}

#[cfg(test)]
mod tests {
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
}
