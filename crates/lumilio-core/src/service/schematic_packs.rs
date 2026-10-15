//! The schematic studio's sources (plan schematic-studio, T17, T21): resource
//! packs added from elsewhere, whose paths only are kept, and which mods and
//! packs each studio folder draws with. Both live in launcher data.

use std::path::PathBuf;

use super::support::blocking_value;
use super::{LauncherService, ServiceError};
use crate::schematic_assets::{LocalCache, add_pack, forget_switch, remove_pack, switch};
use crate::transfer::Transport;

impl<T: Transport + Clone> LauncherService<T> {
    /// Adds the resource-pack zip at `pack` to `instance`'s studio sources
    /// and switches it on for the studio folder `folder` it was added from.
    /// The file is not copied; the next preview there layers it over the
    /// others.
    pub async fn schematic_pack_add(
        &self,
        instance: &str,
        folder: &str,
        pack: PathBuf,
    ) -> Result<(), ServiceError> {
        self.instance(instance).await?;
        let (list, switches) = (
            self.layout.schematic_packs(instance),
            self.layout.schematic_sources(instance),
        );
        let folder = folder.to_owned();
        blocking_value(move || {
            add_pack(&list, &pack)?;
            switch(&switches, &folder, &pack, true)
        })
        .await?
        .map_err(ServiceError::Io)
    }

    /// Takes `pack` off `instance`'s studio sources, switches it off in every
    /// folder and drops the cached bakes made with it. The file itself is
    /// left alone.
    pub async fn schematic_pack_remove(
        &self,
        instance: &str,
        pack: PathBuf,
    ) -> Result<(), ServiceError> {
        self.instance(instance).await?;
        let (list, switches) = (
            self.layout.schematic_packs(instance),
            self.layout.schematic_sources(instance),
        );
        let cache = LocalCache::new(&self.layout.model_cache());
        blocking_value(move || {
            let removed = remove_pack(&list, &pack).map(drop);
            let forgotten = forget_switch(&switches, &pack);
            cache.forget(&pack);
            removed.and(forgotten)
        })
        .await?
        .map_err(ServiceError::Io)
    }

    /// Switches the mod or resource pack `source` on or off for `instance`'s
    /// studio folder `folder` (T21, D23): every overlay is off until switched
    /// on, and the switch is kept. A path the instance does not offer is
    /// kept too but draws nothing.
    pub async fn schematic_source_set(
        &self,
        instance: &str,
        folder: &str,
        source: PathBuf,
        on: bool,
    ) -> Result<(), ServiceError> {
        self.instance(instance).await?;
        let switches = self.layout.schematic_sources(instance);
        let folder = folder.to_owned();
        blocking_value(move || switch(&switches, &folder, &source, on))
            .await?
            .map_err(ServiceError::Io)
    }
}
