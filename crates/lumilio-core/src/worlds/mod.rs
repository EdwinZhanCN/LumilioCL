//! What a game keeps in its folder besides mods: saved worlds and their map,
//! the server list, screenshots and schematics.

pub(crate) use operations::Worlds;
pub use saves::{
    GameMode, WorldError, WorldInfo, copy_name, delete, duplicate, export_zip, import_zip, scan,
    size, sweep_temporary,
};

pub use self::models::ModelPreview;

mod map;
mod models;
mod operations;
mod saves;
mod schematic_packs;
mod schematics;
mod screenshots;
mod servers;
