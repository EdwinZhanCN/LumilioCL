//! The schematic studio's sources (plan schematic-studio, T17, T21, T36):
//! resource packs added from elsewhere, whose paths only are kept, which mods
//! and packs each studio folder draws with, and the order they are layered
//! in. All live in launcher data.

use std::path::PathBuf;

use super::Worlds;
use crate::blocking::blocking_value;
use crate::error::ServiceError;
use crate::schematic_assets::{
    LocalCache, add_pack, forget_switch, instance_assets, read_order, remove_pack, swapped_sources,
    switch, write_order,
};
use crate::transfer::Transport;

impl<T: Transport + Clone> Worlds<T> {
    /// Adds the resource pack at `pack` (a zip or an unpacked pack folder) to
    /// `instance`'s studio sources and switches it on for the studio folder
    /// `folder` it was added from. The file is not copied; the next preview
    /// there layers it over the others, on top of a kept order (T36).
    pub(crate) async fn schematic_pack_add(
        &self,
        instance: &str,
        folder: &str,
        pack: PathBuf,
    ) -> Result<(), ServiceError> {
        self.instances.record(instance).await?;
        let (list, switches, order) = (
            self.layout.schematic_packs(instance),
            self.layout.schematic_sources(instance),
            self.layout.schematic_order(instance),
        );
        let folder = folder.to_owned();
        blocking_value(move || {
            add_pack(&list, &pack)?;
            let mut kept = read_order(&order);
            if !kept.is_empty() && !kept.contains(&pack) {
                kept.push(pack.clone());
                write_order(&order, &kept)?;
            }
            switch(&switches, &folder, &pack, true)
        })
        .await?
        .map_err(ServiceError::Io)
    }

    /// Takes `pack` off `instance`'s studio sources, switches it off in every
    /// folder and drops the cached bakes made with it. The file itself is
    /// left alone.
    pub(crate) async fn schematic_pack_remove(
        &self,
        instance: &str,
        pack: PathBuf,
    ) -> Result<(), ServiceError> {
        self.instances.record(instance).await?;
        let (list, switches, order) = (
            self.layout.schematic_packs(instance),
            self.layout.schematic_sources(instance),
            self.layout.schematic_order(instance),
        );
        let cache = LocalCache::new(&self.layout.model_cache());
        blocking_value(move || {
            let removed = remove_pack(&list, &pack).map(drop);
            let forgotten = forget_switch(&switches, &pack);
            let mut kept = read_order(&order);
            let unordered = if kept.contains(&pack) {
                kept.retain(|known| known != &pack);
                write_order(&order, &kept)
            } else {
                Ok(())
            };
            cache.forget(&pack);
            removed.and(forgotten).and(unordered)
        })
        .await?
        .map_err(ServiceError::Io)
    }

    /// Switches the mod or resource pack `source` on or off for `instance`'s
    /// studio folder `folder` (T21, D23): every overlay is off until switched
    /// on, and the switch is kept. A path the instance does not offer is
    /// kept too but draws nothing.
    pub(crate) async fn schematic_source_set(
        &self,
        instance: &str,
        folder: &str,
        source: PathBuf,
        on: bool,
    ) -> Result<(), ServiceError> {
        self.instances.record(instance).await?;
        let switches = self.layout.schematic_sources(instance);
        let folder = folder.to_owned();
        blocking_value(move || switch(&switches, &folder, &source, on))
            .await?
            .map_err(ServiceError::Io)
    }

    /// Moves the mod or resource pack `source` past `other`, its neighbour in
    /// the studio's list, in `instance`'s studio order and keeps the whole
    /// order (T36). The order is part of an overlay bake's key, so the next
    /// preview bakes the new stack and the bake in the old order is never
    /// read for it. Returns whether it moved.
    pub(crate) async fn schematic_source_move(
        &self,
        instance: &str,
        source: PathBuf,
        other: PathBuf,
    ) -> Result<bool, ServiceError> {
        let record = self.instances.record(instance).await?;
        let layout = self.layout.clone();
        blocking_value(move || {
            let offered = instance_assets(&layout, &record);
            match swapped_sources(&offered.overlays, &source, &other) {
                Some(order) => {
                    write_order(&layout.schematic_order(&record.id), &order).map(|()| true)
                }
                None => Ok(false),
            }
        })
        .await?
        .map_err(ServiceError::Io)
    }
}
