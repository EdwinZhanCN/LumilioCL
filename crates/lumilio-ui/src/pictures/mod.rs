//! Pictures loaded by path or address (screenshots, project icons, gallery
//! and description pictures, the wallpaper) held within a byte budget.
//!
//! Without a cache, GPUI keeps every such picture for the life of the app at
//! full resolution, in memory and again as a GPU texture. This cache decodes a
//! picture no larger than its edge, counts what it holds and, past its budget,
//! lets go of the pictures drawn least recently, GPU texture included. A
//! picture asked for in a window's latest frame is on screen and is never let
//! go: a page whose own pictures exceed the budget keeps them while it shows.
//!
//! The shell wraps the window in [`scope`]. Dialogs and sheets are drawn by the
//! window root beside the shell, so one that shows pictures wraps them in
//! [`scope`] too.

mod decode;
#[cfg(test)]
mod tests;

#[cfg(test)]
use std::sync::Arc;

#[cfg(test)]
use gpui::{Entity, RenderImage, Resource, Window, image_cache};
#[cfg(test)]
use picture_cache::State;
pub use picture_cache::{PictureCache, pages, release_unseen, scope, wallpaper};

mod picture_cache;
