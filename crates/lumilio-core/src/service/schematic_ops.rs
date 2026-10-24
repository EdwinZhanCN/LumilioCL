//! The studio's writes to an instance's schematics (plan schematic-studio, D8).
//!
//! Files are reached through a plugin's grants (`ReadGameFiles` to read,
//! `WriteGameFiles` to write), so the studio can touch only what the plugin
//! declared and the person allowed. Each write holds the instance, which a
//! running game also holds, so nothing is written while the game runs.

use lumilio_plugin_api::FileInfo;

use super::LauncherService;
use crate::blocking::blocking_value;
use crate::clock::now;
use crate::error::ServiceError;
use crate::plugins::{read_info, resolve_write};
use crate::schematic_ops;
use crate::schematic_ops::{SchematicLogEntry, SchematicOperation};
use crate::transfer::Transport;

impl<T: Transport + Clone> LauncherService<T> {
    /// The studio's saved replacement tables and recent targets (T24).
    pub async fn schematic_schemes(&self) -> Result<crate::ReplaceSchemes, ServiceError> {
        let path = self.layout.replace_schemes();
        blocking_value(move || crate::schematic_ops::schemes::read(&path)).await
    }

    /// Keeps `schemes` as the studio's tables and recent targets.
    pub async fn schematic_schemes_save(
        &self,
        schemes: crate::ReplaceSchemes,
    ) -> Result<crate::ReplaceSchemes, ServiceError> {
        let path = self.layout.replace_schemes();
        blocking_value(move || {
            crate::schematic_ops::schemes::write(&path, &schemes).map(|()| schemes)
        })
        .await?
        .map_err(ServiceError::Io)
    }

    /// Whether a schematic write could start now: no game running on the
    /// instance and no other operation holding it. The studio disables its
    /// write actions while this is false.
    pub fn schematic_can_write(&self, instance: &str) -> bool {
        self.instances.reserve(instance).is_ok()
    }

    /// The files below `folder` that `plugin` may read, relative to the game
    /// directory: the studio's directory tree.
    pub async fn schematic_files(
        &self,
        instance: &str,
        plugin: &str,
        folder: &str,
    ) -> Result<Vec<String>, ServiceError> {
        self.instance(instance).await?;
        self.plugins
            .list_granted(plugin, self.layout.game(instance), folder)
            .await
            .map_err(ServiceError::Plugin)
    }

    /// A schematic's bytes and the [`FileInfo`] a write of it must present.
    /// `file` is relative to the game directory and must be readable under
    /// one of `plugin`'s grants.
    pub async fn schematic_read(
        &self,
        instance: &str,
        plugin: &str,
        file: &str,
    ) -> Result<(Vec<u8>, FileInfo), ServiceError> {
        self.instance(instance).await?;
        let manifest = self.plugins.granted(plugin).await?;
        let (game, file) = (self.layout.game(instance), file.to_owned());
        Ok(blocking_value(move || read_info(&game, &manifest, &file)).await??)
    }

    /// Replaces a schematic with the edited `bytes`. `expected` is what
    /// [`schematic_read`](Self::schematic_read) returned: if the file has
    /// changed since, nothing is written. The old file is backed up and the
    /// operation logged first-to-last; the returned entry is the new log line.
    pub async fn schematic_apply(
        &self,
        instance: &str,
        plugin: &str,
        file: &str,
        bytes: Vec<u8>,
        expected: Option<FileInfo>,
        operation: SchematicOperation,
    ) -> Result<SchematicLogEntry, ServiceError> {
        self.instance(instance).await?;
        let manifest = self.plugins.granted(plugin).await?;
        let _lease = self.instances.reserve(instance)?;
        let (game, folder) = (
            self.layout.game(instance),
            self.layout.schematic_ops(instance),
        );
        let file = file.to_owned();
        Ok(blocking_value(move || {
            let target = resolve_write(&game, &manifest, &file)?;
            schematic_ops::apply(
                &folder,
                &target,
                &file,
                &bytes,
                expected.as_ref(),
                &operation,
                now(),
            )
        })
        .await??)
    }

    /// Moves schematics into the launcher's backups, one log line each, under
    /// one hold of the instance: deleting and "keep only this" never delete.
    /// Each `(file, expected)` is refused if the file changed since it was
    /// read; the files before a refusal stay moved (each is restorable).
    pub async fn schematic_remove(
        &self,
        instance: &str,
        plugin: &str,
        files: Vec<(String, FileInfo)>,
        operation: SchematicOperation,
    ) -> Result<Vec<SchematicLogEntry>, ServiceError> {
        self.instance(instance).await?;
        let manifest = self.plugins.granted(plugin).await?;
        let _lease = self.instances.reserve(instance)?;
        let (game, folder) = (
            self.layout.game(instance),
            self.layout.schematic_ops(instance),
        );
        Ok(blocking_value(move || {
            files
                .iter()
                .map(|(file, expected)| {
                    let target = resolve_write(&game, &manifest, file)?;
                    schematic_ops::remove(&folder, &target, file, expected, &operation, now())
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .await??)
    }

    /// The instance's schematic operations, newest first.
    pub async fn schematic_history(
        &self,
        instance: &str,
    ) -> Result<Vec<SchematicLogEntry>, ServiceError> {
        self.instance(instance).await?;
        let folder = self.layout.schematic_ops(instance);
        Ok(blocking_value(move || schematic_ops::history(&folder)).await??)
    }

    /// Every copy in the instance's schematic backups, newest first (T40):
    /// the history panel matches them to log lines and lists the rest.
    pub async fn schematic_backups(
        &self,
        instance: &str,
    ) -> Result<Vec<schematic_ops::KeptBackup>, ServiceError> {
        self.instance(instance).await?;
        let folder = self.layout.schematic_ops(instance);
        Ok(blocking_value(move || schematic_ops::backups(&folder)).await??)
    }

    /// Puts the kept copy `backup` (a [`schematic_ops::KeptBackup::path`])
    /// back over the file it is a copy of, for a copy no log line names
    /// (T40). The file must be writable under `plugin`'s grants; the restore
    /// is logged like any write.
    pub async fn schematic_restore_backup(
        &self,
        instance: &str,
        plugin: &str,
        backup: String,
    ) -> Result<SchematicLogEntry, ServiceError> {
        self.instance(instance).await?;
        let manifest = self.plugins.granted(plugin).await?;
        let _lease = self.instances.reserve(instance)?;
        let (game, folder) = (
            self.layout.game(instance),
            self.layout.schematic_ops(instance),
        );
        Ok(blocking_value(move || {
            let kept = schematic_ops::backups(&folder)?
                .into_iter()
                .find(|kept| kept.path == backup)
                .ok_or_else(|| {
                    lumilio_plugin_api::PluginError::InvalidInput("no such backup".into())
                })?;
            let target = resolve_write(&game, &manifest, &kept.file)?;
            schematic_ops::restore_backup(&folder, &target, &kept.file, &kept.path, now())
        })
        .await??)
    }

    /// Puts back the file as it was before the operation on log line `index`,
    /// from the launcher's own backup. The restore is logged like any write.
    pub async fn schematic_restore(
        &self,
        instance: &str,
        plugin: &str,
        index: usize,
    ) -> Result<SchematicLogEntry, ServiceError> {
        self.instance(instance).await?;
        let manifest = self.plugins.granted(plugin).await?;
        let _lease = self.instances.reserve(instance)?;
        let (game, folder) = (
            self.layout.game(instance),
            self.layout.schematic_ops(instance),
        );
        Ok(blocking_value(move || {
            let entry = schematic_ops::entry(&folder, index)?;
            let target = resolve_write(&game, &manifest, &entry.file)?;
            schematic_ops::restore(&folder, &target, &entry, now())
        })
        .await??)
    }
}
