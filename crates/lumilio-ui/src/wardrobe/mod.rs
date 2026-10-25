//! Retained wardrobe for one account detail. The Accounts feature owns I/O;
//! this entity owns pending feedback and asks for operations through the
//! handler the feature bound to its detail.
//! It lays out the detail's look: the preview with what can be done under
//! it, beside the library as pictures drawn by the CPU skin renderer. A skin
//! is tried on in the preview before it is worn.

use std::collections::HashMap;
use std::sync::Arc;

#[cfg(test)]
use gpui::prelude::*;
use gpui::{
    App, Context, IntoElement, ObjectFit, Render, RenderImage, WeakEntity, Window, div, img, px,
};
use gpui_component::{h_flex, v_flex};
use lumilio_core::{AccountLook, LibrarySkin, MojangProfile, SkinModel};
use lumilio_skin_render::{Arms, Camera, Player, Texture, render};
pub use wardrobe_view::{Wardrobe, WardrobeAction, WardrobeHandler};

use self::pictures::{CAPE_THUMB, SKIN_THUMB, arms, texture};
use self::render::tile;
pub(crate) use self::render::{PINNED_PREVIEW_WIDTH, PREVIEW_WIDTH};
use crate::key::Key;
use crate::skin_view::Trial;
use crate::theme::ShellColors;
use crate::{kit, tr, tr_all};

mod editor;
mod pictures;
mod render;

#[cfg(test)]
mod tests;

mod wardrobe_view;
