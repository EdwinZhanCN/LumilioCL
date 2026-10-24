//! Skins for offline players, and the little server that hands them to the
//! game (through authlib-injector) so it shows them.
//!
//! The rules are adapted from HMCL (`HMCLCore/.../auth/offline/Skin.java`,
//! `auth/offline/YggdrasilServer.java` and `auth/yggdrasil/Texture.java`,
//! Copyright (C) 2020–2021 huangyuhui and contributors, GPL-3.0-or-later; ADR
//! 0011): a skin is a local file, a LittleSkin profile or a CustomSkinLoader
//! API; the game is pointed at a Yggdrasil server on this machine that answers
//! for that one player.

pub(crate) mod cache;
pub(crate) mod defaults;
mod library;
mod mojang;
mod pixels;
mod server;
pub(crate) mod session;
mod signer;

#[cfg(test)]
use std::{fs, io};

#[cfg(test)]
use image::{ImageFormat, RgbaImage};
pub use textures::{LITTLE_SKIN_CSL, LoadedSkin, SkinChoice, SkinError, SkinModel, check, load};
use textures::{MAX_SIDE, PICTURE_LIMIT};
#[cfg(test)]
pub(crate) use textures::{Texture, normalize_api};

pub use self::library::{LibrarySkin, PairedCape, SkinLibrary, SkinSource};
pub use self::mojang::{
    AppearanceChange, AppearanceError, AppearanceUpdate, MojangCape, MojangClient, MojangProfile,
    MojangSkin,
};
pub use self::pixels::{AccountLook, Pixels, cape_pixels, face, looks_slim, skin_pixels};
pub use self::server::{Character, LocalSkinServer};
pub(crate) use self::signer::KEY_BITS;
pub use self::signer::Signer;
#[cfg(test)]
use crate::transfer::{TransferEngine, Transport};

#[cfg(test)]
mod tests;

mod textures;
