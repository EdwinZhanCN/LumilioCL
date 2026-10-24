use std::path::{Path, PathBuf};

use lumilio_block_assets::{BlockCatalog, BlockNames};
use lumilio_plugin_api::FileInfo;
use lumilio_plugin_api::map::{
    Dimension, MapObject, OverlayInfo, OverlayRequest, TileReply, TileRequest, WorldContext,
    WorldId,
};

use super::LauncherService;
use crate::activity::CancellationToken;
use crate::error::ServiceError;
use crate::schematic_ops::{SchematicLogEntry, SchematicOperation};
use crate::screenshots::ScreenshotInfo;
use crate::servers::{ServerEntry, ServerStatus};
use crate::world_map::store::annotations::Annotation;
use crate::worlds::{ModelPreview, WorldInfo};
use crate::{MapFailure, MapProviders, Transport, schematic_ops, world_map};

impl<T: Transport + Clone> LauncherService<T> {
    pub async fn worlds(&self, id: &str) -> Result<Vec<WorldInfo>, ServiceError> {
        self.worlds.worlds(id).await
    }

    pub async fn copy_world(
        &self,
        id: &str,
        folder: &str,
        new_folder: Option<&str>,
    ) -> Result<String, ServiceError> {
        self.worlds.copy_world(id, folder, new_folder).await
    }

    pub async fn delete_world(&self, id: &str, folder: &str) -> Result<(), ServiceError> {
        self.worlds.delete_world(id, folder).await
    }

    pub async fn export_world(
        &self,
        id: &str,
        folder: &str,
        destination: &Path,
    ) -> Result<u64, ServiceError> {
        self.worlds.export_world(id, folder, destination).await
    }

    pub async fn export_modpack(
        &self,
        id: &str,
        spec: crate::pack_export::ExportSpec,
        destination: &Path,
        cancel: CancellationToken,
    ) -> Result<crate::pack_export::ExportReport, ServiceError> {
        self.worlds
            .export_modpack(id, spec, destination, cancel)
            .await
    }

    pub async fn import_world(&self, id: &str, archive: &Path) -> Result<String, ServiceError> {
        self.worlds.import_world(id, archive).await
    }

    pub async fn servers(&self, id: &str) -> Result<Vec<ServerEntry>, ServiceError> {
        self.worlds.servers(id).await
    }

    pub async fn add_server(&self, id: &str, entry: ServerEntry) -> Result<(), ServiceError> {
        self.worlds.add_server(id, entry).await
    }

    pub async fn update_server(
        &self,
        id: &str,
        index: usize,
        expected: ServerEntry,
        entry: ServerEntry,
    ) -> Result<(), ServiceError> {
        self.worlds.update_server(id, index, expected, entry).await
    }

    pub async fn remove_server(
        &self,
        id: &str,
        index: usize,
        expected: ServerEntry,
    ) -> Result<(), ServiceError> {
        self.worlds.remove_server(id, index, expected).await
    }

    pub async fn move_server(
        &self,
        id: &str,
        index: usize,
        expected: ServerEntry,
        to: usize,
    ) -> Result<(), ServiceError> {
        self.worlds.move_server(id, index, expected, to).await
    }

    pub async fn server_status(
        &self,
        id: &str,
        address: &str,
    ) -> Result<ServerStatus, ServiceError> {
        self.worlds.server_status(id, address).await
    }

    pub async fn screenshots(&self, id: &str) -> Result<Vec<ScreenshotInfo>, ServiceError> {
        self.worlds.screenshots(id).await
    }

    pub async fn screenshot_thumbnail(
        &self,
        id: &str,
        file: &str,
    ) -> Result<PathBuf, ServiceError> {
        self.worlds.screenshot_thumbnail(id, file).await
    }

    pub async fn screenshot_bytes(&self, id: &str, file: &str) -> Result<Vec<u8>, ServiceError> {
        self.worlds.screenshot_bytes(id, file).await
    }

    pub async fn delete_screenshot(&self, id: &str, file: &str) -> Result<(), ServiceError> {
        self.worlds.delete_screenshot(id, file).await
    }

    pub async fn map_annotations(
        &self,
        instance: &str,
        world: &WorldId,
        dimension: &Dimension,
    ) -> Result<Vec<Annotation>, ServiceError> {
        self.worlds
            .map_annotations(instance, world, dimension)
            .await
    }

    pub async fn put_map_annotation(
        &self,
        instance: &str,
        annotation: Annotation,
    ) -> Result<i64, ServiceError> {
        self.worlds.put_map_annotation(instance, annotation).await
    }

    pub async fn remove_map_annotation(
        &self,
        instance: &str,
        id: i64,
    ) -> Result<bool, ServiceError> {
        self.worlds.remove_map_annotation(instance, id).await
    }

    pub async fn map_contexts(
        &self,
        instance: &str,
    ) -> Result<Vec<world_map::WorldMapContext>, ServiceError> {
        self.worlds.map_contexts(instance).await
    }

    pub async fn link_map_xaero(
        &self,
        instance: &str,
        folder: &str,
        dir: &str,
    ) -> Result<(), ServiceError> {
        self.worlds.link_map_xaero(instance, folder, dir).await
    }

    pub async fn unlink_map_xaero(&self, instance: &str, folder: &str) -> Result<(), ServiceError> {
        self.worlds.unlink_map_xaero(instance, folder).await
    }

    pub async fn map_providers(&self) -> MapProviders {
        self.worlds.map_providers().await
    }

    pub async fn map_overlays(&self, context: &WorldContext) -> Vec<(String, OverlayInfo)> {
        self.worlds.map_overlays(context).await
    }

    pub async fn map_objects(
        &self,
        instance: &str,
        plugin: &str,
        request: OverlayRequest,
        cancel: CancellationToken,
    ) -> Result<Vec<MapObject>, MapFailure> {
        self.worlds
            .map_objects(instance, plugin, request, cancel)
            .await
    }

    pub async fn save_map_seed(
        &self,
        instance: &str,
        seed: i64,
        version: &str,
    ) -> Result<(), ServiceError> {
        self.worlds.save_map_seed(instance, seed, version).await
    }

    pub async fn map_tile(
        &self,
        instance: &str,
        request: TileRequest,
        cancel: CancellationToken,
    ) -> Result<TileReply, MapFailure> {
        self.worlds.map_tile(instance, request, cancel).await
    }

    pub fn map_can_edit(&self, instance: &str) -> bool {
        self.worlds.map_can_edit(instance)
    }

    pub async fn map_apply(
        &self,
        instance: &str,
        plugin: &str,
        edit: lumilio_plugin_api::map::ObjectEdit,
    ) -> Result<(), MapFailure> {
        self.worlds.map_apply(instance, plugin, edit).await
    }

    pub async fn clear_map_cache(&self) -> Result<(), ServiceError> {
        self.worlds.clear_map_cache().await
    }

    pub async fn schematic_schemes(&self) -> Result<crate::ReplaceSchemes, ServiceError> {
        self.worlds.schematic_schemes().await
    }

    pub async fn schematic_schemes_save(
        &self,
        schemes: crate::ReplaceSchemes,
    ) -> Result<crate::ReplaceSchemes, ServiceError> {
        self.worlds.schematic_schemes_save(schemes).await
    }

    pub fn schematic_can_write(&self, instance: &str) -> bool {
        self.worlds.schematic_can_write(instance)
    }

    pub async fn schematic_files(
        &self,
        instance: &str,
        plugin: &str,
        folder: &str,
    ) -> Result<Vec<String>, ServiceError> {
        self.worlds.schematic_files(instance, plugin, folder).await
    }

    pub async fn schematic_read(
        &self,
        instance: &str,
        plugin: &str,
        file: &str,
    ) -> Result<(Vec<u8>, FileInfo), ServiceError> {
        self.worlds.schematic_read(instance, plugin, file).await
    }

    pub async fn schematic_apply(
        &self,
        instance: &str,
        plugin: &str,
        file: &str,
        bytes: Vec<u8>,
        expected: Option<FileInfo>,
        operation: SchematicOperation,
    ) -> Result<SchematicLogEntry, ServiceError> {
        self.worlds
            .schematic_apply(instance, plugin, file, bytes, expected, operation)
            .await
    }

    pub async fn schematic_remove(
        &self,
        instance: &str,
        plugin: &str,
        files: Vec<(String, FileInfo)>,
        operation: SchematicOperation,
    ) -> Result<Vec<SchematicLogEntry>, ServiceError> {
        self.worlds
            .schematic_remove(instance, plugin, files, operation)
            .await
    }

    pub async fn schematic_history(
        &self,
        instance: &str,
    ) -> Result<Vec<SchematicLogEntry>, ServiceError> {
        self.worlds.schematic_history(instance).await
    }

    pub async fn schematic_backups(
        &self,
        instance: &str,
    ) -> Result<Vec<schematic_ops::KeptBackup>, ServiceError> {
        self.worlds.schematic_backups(instance).await
    }

    pub async fn schematic_restore_backup(
        &self,
        instance: &str,
        plugin: &str,
        backup: String,
    ) -> Result<SchematicLogEntry, ServiceError> {
        self.worlds
            .schematic_restore_backup(instance, plugin, backup)
            .await
    }

    pub async fn schematic_restore(
        &self,
        instance: &str,
        plugin: &str,
        index: usize,
    ) -> Result<SchematicLogEntry, ServiceError> {
        self.worlds.schematic_restore(instance, plugin, index).await
    }

    pub async fn schematic_pack_add(
        &self,
        instance: &str,
        folder: &str,
        pack: PathBuf,
    ) -> Result<(), ServiceError> {
        self.worlds.schematic_pack_add(instance, folder, pack).await
    }

    pub async fn schematic_pack_remove(
        &self,
        instance: &str,
        pack: PathBuf,
    ) -> Result<(), ServiceError> {
        self.worlds.schematic_pack_remove(instance, pack).await
    }

    pub async fn schematic_source_set(
        &self,
        instance: &str,
        folder: &str,
        source: PathBuf,
        on: bool,
    ) -> Result<(), ServiceError> {
        self.worlds
            .schematic_source_set(instance, folder, source, on)
            .await
    }

    pub async fn schematic_source_move(
        &self,
        instance: &str,
        source: PathBuf,
        other: PathBuf,
    ) -> Result<bool, ServiceError> {
        self.worlds
            .schematic_source_move(instance, source, other)
            .await
    }

    pub async fn plugin_model(
        &self,
        instance: &str,
        plugin: &str,
        file: &str,
    ) -> Result<ModelPreview, ServiceError> {
        self.worlds.plugin_model(instance, plugin, file).await
    }

    pub async fn schematic_names(&self, instance: &str) -> Result<BlockNames, ServiceError> {
        self.worlds.schematic_names(instance).await
    }

    pub async fn schematic_thumbnail<D>(
        &self,
        instance: &str,
        plugin: &str,
        file: &str,
        key: &str,
        draw: D,
    ) -> Result<Vec<u8>, ServiceError>
    where
        D: FnOnce(ModelPreview) -> Result<Vec<u8>, String> + Send + 'static,
    {
        self.worlds
            .schematic_thumbnail(instance, plugin, file, key, draw)
            .await
    }

    pub async fn schematic_versions(&self) -> Result<Vec<(String, u32)>, ServiceError> {
        self.worlds.schematic_versions().await
    }

    pub async fn schematic_catalogs(
        &self,
        target: &str,
        source_data_version: u32,
    ) -> Result<(BlockCatalog, Option<BlockCatalog>), ServiceError> {
        self.worlds
            .schematic_catalogs(target, source_data_version)
            .await
    }
}
