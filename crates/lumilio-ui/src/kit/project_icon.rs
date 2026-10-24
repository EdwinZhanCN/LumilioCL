//! A project's icon, fetched by address, over a placeholder cover.

use gpui::prelude::*;
use gpui::{ObjectFit, div, img, px};

use crate::cover::Loader;
use crate::home::WorldHint;
use crate::theme::ShellColors;

/// The placeholder cover: shown until the real icon arrives, and instead of
/// it when a project has none or the picture cannot be loaded.
fn placeholder(seed: u32, colors: ShellColors) -> gpui::AnyElement {
    const LOADERS: [Loader; 5] = [
        Loader::Fabric,
        Loader::Forge,
        Loader::NeoForge,
        Loader::Quilt,
        Loader::Vanilla,
    ];
    const WORLDS: [WorldHint; 4] = [
        WorldHint::Overworld,
        WorldHint::Underground,
        WorldHint::Redstone,
        WorldHint::Nether,
    ];
    div()
        .size_full()
        .child(crate::cover::element(
            seed,
            LOADERS[seed as usize % LOADERS.len()],
            WORLDS[seed as usize / 5 % WORLDS.len()],
            colors.surface,
            px(0.),
        ))
        .into_any_element()
}

/// A project's icon at `side` pixels, fetched by address.
pub fn project_icon(
    url: Option<&str>,
    seed: u32,
    side: f32,
    colors: ShellColors,
) -> gpui::AnyElement {
    let frame = div()
        .flex_none()
        .size(px(side))
        .rounded(px(side / 6.))
        .overflow_hidden()
        .bg(colors.surface_subtle);
    match url {
        Some(url) => frame
            .child(
                img(url.to_owned())
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .with_loading(move || placeholder(seed, colors))
                    .with_fallback(move || placeholder(seed, colors)),
            )
            .into_any_element(),
        None => frame.child(placeholder(seed, colors)).into_any_element(),
    }
}
