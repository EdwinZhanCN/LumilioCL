//! The map's structure icons. Provenance and status: `assets/map-icons/NOTICE.md`.
//! This is the only place that names the files, so removing them is deleting
//! that directory and the table below; an icon without a file draws a dot.
use gpui::{Image, ImageFormat};
use lumilio_plugin_api::map::MapIcon;
use std::sync::{Arc, OnceLock};

macro_rules! icon {
    ($name:literal) => {
        include_bytes!(concat!("../assets/map-icons/", $name, ".png")).as_slice()
    };
}

fn bytes(icon: MapIcon) -> Option<&'static [u8]> {
    Some(match icon {
        MapIcon::Village => icon!("village"),
        MapIcon::DesertPyramid => icon!("desert_pyramid"),
        MapIcon::JungleTemple => icon!("jungle_temple"),
        MapIcon::SwampHut => icon!("swamp_hut"),
        MapIcon::Igloo => icon!("igloo"),
        MapIcon::OceanRuin => icon!("ocean_ruin"),
        MapIcon::Shipwreck => icon!("shipwreck"),
        MapIcon::Monument => icon!("ocean_monument"),
        MapIcon::Mansion => icon!("woodland_mansion"),
        MapIcon::Outpost => icon!("pillager_outpost"),
        MapIcon::RuinedPortal => icon!("ruined_portal"),
        MapIcon::AncientCity => icon!("ancient_city"),
        MapIcon::TrailRuins => icon!("trail_ruin"),
        MapIcon::TrialChambers => icon!("trial_chambers"),
        MapIcon::Stronghold => icon!("stronghold"),
        MapIcon::Fortress => icon!("nether_fortress"),
        MapIcon::Bastion => icon!("bastion_remnant"),
        MapIcon::EndCity => icon!("end_city"),
        MapIcon::Spawn => icon!("spawn_point"),
        MapIcon::SlimeChunk => icon!("slime_chunks"),
        MapIcon::Waypoint | MapIcon::Death | MapIcon::Marker | MapIcon::Player => return None,
    })
}

/// The decoded image for an icon, shared across frames, or `None` when it has
/// no file.
pub(crate) fn image(icon: MapIcon) -> Option<Arc<Image>> {
    type Decoded = std::sync::Mutex<Vec<(MapIcon, Arc<Image>)>>;
    static CACHE: OnceLock<Decoded> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let mut cache = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((_, image)) = cache.iter().find(|(known, _)| *known == icon) {
        return Some(image.clone());
    }
    let image = Arc::new(Image::from_bytes(ImageFormat::Png, bytes(icon)?.to_vec()));
    cache.push((icon, image.clone()));
    Some(image)
}

/// An icon's pixels for the map frame: straight-alpha RGBA at its own size.
pub(crate) struct Pixels {
    /// Names the icon to the renderer, which keeps one texture per name.
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

fn name(icon: MapIcon) -> &'static str {
    match icon {
        MapIcon::Village => "village",
        MapIcon::DesertPyramid => "desert-pyramid",
        MapIcon::JungleTemple => "jungle-temple",
        MapIcon::SwampHut => "swamp-hut",
        MapIcon::Igloo => "igloo",
        MapIcon::OceanRuin => "ocean-ruin",
        MapIcon::Shipwreck => "shipwreck",
        MapIcon::Monument => "monument",
        MapIcon::Mansion => "mansion",
        MapIcon::Outpost => "outpost",
        MapIcon::RuinedPortal => "ruined-portal",
        MapIcon::AncientCity => "ancient-city",
        MapIcon::TrailRuins => "trail-ruins",
        MapIcon::TrialChambers => "trial-chambers",
        MapIcon::Stronghold => "stronghold",
        MapIcon::Fortress => "fortress",
        MapIcon::Bastion => "bastion",
        MapIcon::EndCity => "end-city",
        MapIcon::Spawn => "spawn",
        MapIcon::SlimeChunk => "slime-chunk",
        MapIcon::Waypoint => "waypoint",
        MapIcon::Death => "death",
        MapIcon::Marker => "marker",
        MapIcon::Player => "player",
    }
}

/// A plain dot, for an icon with no file.
fn dot(name: String) -> Pixels {
    const EDGE: u32 = 24;
    let centre = (EDGE as f32 - 1.) / 2.;
    let rgba = (0..EDGE * EDGE)
        .flat_map(|at| {
            let (x, z) = ((at % EDGE) as f32 - centre, (at / EDGE) as f32 - centre);
            let edge = (9. - (x * x + z * z).sqrt()).clamp(0., 1.);
            [0xE8, 0x6A, 0x2E, (edge * 255.) as u8]
        })
        .collect();
    Pixels {
        name,
        width: EDGE,
        height: EDGE,
        rgba,
    }
}

/// The decoded pixels of an icon, shared across frames. An icon without a file
/// or that does not decode is a dot.
pub(crate) fn pixels(icon: MapIcon) -> Arc<Pixels> {
    type Decoded = std::sync::Mutex<Vec<(MapIcon, Arc<Pixels>)>>;
    static CACHE: OnceLock<Decoded> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let mut cache = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((_, found)) = cache.iter().find(|(known, _)| *known == icon) {
        return found.clone();
    }
    let decoded = bytes(icon)
        .and_then(|bytes| image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok())
        .map(|decoded| {
            let rgba = decoded.to_rgba8();
            Pixels {
                name: name(icon).to_owned(),
                width: rgba.width(),
                height: rgba.height(),
                rgba: rgba.into_raw().into(),
            }
        })
        .unwrap_or_else(|| dot(name(icon).to_owned()));
    let found = Arc::new(decoded);
    cache.push((icon, found.clone()));
    found
}

/// How a data-coloured marker is shaped.
#[derive(Clone, Copy)]
enum Shape {
    Disc,
    Cross,
    Diamond,
    Ring,
}

/// Signed distance from the shape's edge at `(x, z)` from the glyph's centre:
/// negative inside.
fn distance(shape: Shape, x: f32, z: f32) -> f32 {
    match shape {
        Shape::Disc => (x * x + z * z).sqrt() - 7.5,
        Shape::Diamond => (x.abs() + z.abs()) / std::f32::consts::SQRT_2 - 7.5,
        // A ring 3 px wide, open in the middle so the ground shows through.
        Shape::Ring => ((x * x + z * z).sqrt() - 6.).abs() - 1.5,
        // Two diagonal strokes, each 3.6 px wide, clipped to the glyph's box.
        Shape::Cross => {
            let reach = x.abs().max(z.abs()) - 7.5;
            let stroke = |across: f32| (across.abs() / std::f32::consts::SQRT_2 - 1.8).max(reach);
            stroke(x - z).min(stroke(x + z))
        }
    }
}

/// A marker in a colour the data chose: the shape filled with `color` inside a
/// dark outline, so it reads on any biome.
fn glyph(shape: Shape, color: [u8; 3], name: String) -> Pixels {
    const EDGE: u32 = 24;
    let centre = (EDGE as f32 - 1.) / 2.;
    let rgba = (0..EDGE * EDGE)
        .flat_map(|at| {
            let (x, z) = ((at % EDGE) as f32 - centre, (at / EDGE) as f32 - centre);
            let d = distance(shape, x, z);
            let fill = (0.5 - d).clamp(0., 1.);
            let outline = (0.5 - (d - 2.)).clamp(0., 1.);
            // Outline under, colour over it, both anti-aliased.
            let alpha = outline.max(fill);
            let mix = if alpha > 0. { fill / alpha } else { 0. };
            let channel = |c: u8| (f32::from(c) * mix + 24. * (1. - mix)).round() as u8;
            [
                channel(color[0]),
                channel(color[1]),
                channel(color[2]),
                (alpha * 255.).round() as u8,
            ]
        })
        .collect();
    Pixels {
        name,
        width: EDGE,
        height: EDGE,
        rgba,
    }
}

/// The pixels for a marker drawn in `color`: a disc for a waypoint, a cross for
/// a death point, a diamond for a custom marker, a ring for a player. Icons with artwork ignore the
/// colour and come from [`pixels`].
pub(crate) fn marker(icon: MapIcon, color: [u8; 3]) -> Arc<Pixels> {
    type Made = std::sync::Mutex<Vec<((MapIcon, [u8; 3]), Arc<Pixels>)>>;
    static CACHE: OnceLock<Made> = OnceLock::new();
    let shape = match icon {
        MapIcon::Waypoint => Shape::Disc,
        MapIcon::Death => Shape::Cross,
        MapIcon::Marker => Shape::Diamond,
        MapIcon::Player => Shape::Ring,
        _ => return pixels(icon),
    };
    let cache = CACHE.get_or_init(Default::default);
    let mut cache = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((_, found)) = cache.iter().find(|(known, _)| *known == (icon, color)) {
        return found.clone();
    }
    let [r, g, b] = color;
    let made = Arc::new(glyph(
        shape,
        color,
        format!("{}-{r:02x}{g:02x}{b:02x}", name(icon)),
    ));
    if cache.len() >= 64 {
        cache.remove(0);
    }
    cache.push(((icon, color), made.clone()));
    made
}
