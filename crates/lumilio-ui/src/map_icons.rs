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
        MapIcon::Waypoint | MapIcon::Marker => return None,
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
    pub name: &'static str,
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
        MapIcon::Marker => "marker",
    }
}

/// A plain dot, for an icon with no file.
fn dot(name: &'static str) -> Pixels {
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
                name: name(icon),
                width: rgba.width(),
                height: rgba.height(),
                rgba: rgba.into_raw().into(),
            }
        })
        .unwrap_or_else(|| dot(name(icon)));
    let found = Arc::new(decoded);
    cache.push((icon, found.clone()));
    found
}
