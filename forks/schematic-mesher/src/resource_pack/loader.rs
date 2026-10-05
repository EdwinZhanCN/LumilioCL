//! Resource pack loading from ZIP files and directories.

use super::{BlockModel, BlockstateDefinition, ResourcePack, TextureData};
use crate::error::{MesherError, Result};
use crate::resource_pack::texture::{load_texture_from_bytes, parse_mcmeta};
use std::io::Read;
use std::path::Path;

/// Load a resource pack from a file path.
///
/// Supports both ZIP files and directories.
pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<ResourcePack> {
    let path = path.as_ref();

    if path.is_dir() {
        load_from_directory(path)
    } else {
        let data = std::fs::read(path)?;
        load_from_bytes(&data)
    }
}

/// Load a resource pack from bytes (ZIP data).
pub fn load_from_bytes(data: &[u8]) -> Result<ResourcePack> {
    let cursor = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor)?;

    let mut pack = ResourcePack::new();

    // Collect .png.mcmeta entries to apply after all textures are loaded
    // (ZIP entry order is arbitrary, mcmeta may appear before its PNG)
    let mut pending_mcmeta: Vec<(String, String, crate::resource_pack::texture::AnimationMeta)> = Vec::new();

    // Iterate through all files in the archive
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let file_path = file.name().to_string();

        // Skip directories
        if file.is_dir() {
            continue;
        }

        // Parse the path to determine namespace and type
        if let Some((namespace, asset_type, asset_path)) = parse_asset_path(&file_path) {
            match asset_type {
                "blockstates" => {
                    if asset_path.ends_with(".json") {
                        let mut contents = String::new();
                        file.read_to_string(&mut contents)?;

                        let block_id = asset_path.trim_end_matches(".json");
                        match serde_json::from_str::<BlockstateDefinition>(&contents) {
                            Ok(def) => {
                                pack.add_blockstate(namespace, block_id, def);
                            }
                            Err(e) => {
                                // Log warning but continue
                                eprintln!(
                                    "Warning: Failed to parse blockstate {}/{}: {}",
                                    namespace, block_id, e
                                );
                            }
                        }
                    }
                }
                "models" => {
                    if asset_path.ends_with(".json") {
                        let mut contents = String::new();
                        file.read_to_string(&mut contents)?;

                        let model_path = asset_path.trim_end_matches(".json");
                        match serde_json::from_str::<BlockModel>(&contents) {
                            Ok(model) => {
                                pack.add_model(namespace, model_path, model);
                            }
                            Err(e) => {
                                eprintln!(
                                    "Warning: Failed to parse model {}/{}: {}",
                                    namespace, model_path, e
                                );
                            }
                        }
                    }
                }
                "textures" => {
                    if asset_path.ends_with(".png.mcmeta") {
                        // Parse .mcmeta and defer application until all textures are loaded
                        let mut contents = String::new();
                        file.read_to_string(&mut contents)?;
                        let texture_path = asset_path.trim_end_matches(".png.mcmeta");
                        if let Some(meta) = parse_mcmeta(&contents) {
                            pending_mcmeta.push((namespace.to_string(), texture_path.to_string(), meta));
                        }
                    } else if asset_path.ends_with(".png") {
                        let mut data = Vec::new();
                        file.read_to_end(&mut data)?;

                        let texture_path = asset_path.trim_end_matches(".png");
                        match load_texture_from_bytes(&data) {
                            Ok(texture) => {
                                pack.add_texture(namespace, texture_path, texture);
                            }
                            Err(e) => {
                                eprintln!(
                                    "Warning: Failed to load texture {}/{}: {}",
                                    namespace, texture_path, e
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Apply mcmeta metadata to loaded textures
    for (namespace, texture_path, meta) in pending_mcmeta {
        if let Some(ns_textures) = pack.textures.get_mut(&namespace) {
            if let Some(texture) = ns_textures.get_mut(&texture_path) {
                texture.apply_mcmeta(meta);
            }
        }
    }

    Ok(pack)
}

/// Load a resource pack from a directory.
fn load_from_directory(path: &Path) -> Result<ResourcePack> {
    let mut pack = ResourcePack::new();

    // Look for assets directory
    let assets_path = path.join("assets");
    if !assets_path.exists() {
        return Err(MesherError::InvalidResourcePack(
            "No assets directory found".to_string(),
        ));
    }

    // Iterate through namespaces
    for namespace_entry in std::fs::read_dir(&assets_path)? {
        let namespace_entry = namespace_entry?;
        if !namespace_entry.file_type()?.is_dir() {
            continue;
        }

        let namespace = namespace_entry
            .file_name()
            .to_string_lossy()
            .to_string();
        let namespace_path = namespace_entry.path();

        // Load blockstates
        let blockstates_path = namespace_path.join("blockstates");
        if blockstates_path.exists() {
            load_json_files(&blockstates_path, &namespace, |block_id, contents| {
                if let Ok(def) = serde_json::from_str::<BlockstateDefinition>(contents) {
                    pack.add_blockstate(&namespace, block_id, def);
                }
            })?;
        }

        // Load models
        let models_path = namespace_path.join("models");
        if models_path.exists() {
            load_json_files_recursive(&models_path, &models_path, &namespace, &mut |model_path, contents| {
                if let Ok(model) = serde_json::from_str::<BlockModel>(contents) {
                    pack.add_model(&namespace, model_path, model);
                }
            })?;
        }

        // Load textures
        let textures_path = namespace_path.join("textures");
        if textures_path.exists() {
            load_texture_files_recursive(&textures_path, &textures_path, &namespace, &mut |texture_path, data| {
                if let Ok(texture) = load_texture_from_bytes(data) {
                    pack.add_texture(&namespace, texture_path, texture);
                }
            })?;

            // Load .png.mcmeta files and apply to textures
            let mut pending_mcmeta = Vec::new();
            load_mcmeta_files_recursive(&textures_path, &textures_path, &mut |texture_path, contents| {
                if let Some(meta) = parse_mcmeta(contents) {
                    pending_mcmeta.push((texture_path.to_string(), meta));
                }
            })?;

            for (texture_path, meta) in pending_mcmeta {
                if let Some(ns_textures) = pack.textures.get_mut(&namespace) {
                    if let Some(texture) = ns_textures.get_mut(&texture_path) {
                        texture.apply_mcmeta(meta);
                    }
                }
            }
        }
    }

    Ok(pack)
}

/// Parse an asset path from a ZIP file.
/// Returns (namespace, asset_type, asset_path) if valid.
fn parse_asset_path(file_path: &str) -> Option<(&str, &str, &str)> {
    // Expected format: assets/{namespace}/{type}/{path}
    let parts: Vec<&str> = file_path.splitn(4, '/').collect();

    if parts.len() >= 4 && parts[0] == "assets" {
        Some((parts[1], parts[2], parts[3]))
    } else {
        None
    }
}

/// Load JSON files from a directory.
fn load_json_files<F>(dir: &Path, namespace: &str, mut handler: F) -> Result<()>
where
    F: FnMut(&str, &str),
{
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map(|e| e == "json").unwrap_or(false) {
            let file_name = path
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .to_string();

            let contents = std::fs::read_to_string(&path)?;
            handler(&file_name, &contents);
        }
    }
    Ok(())
}

/// Load JSON files recursively from a directory.
fn load_json_files_recursive<F>(
    base: &Path,
    dir: &Path,
    namespace: &str,
    handler: &mut F,
) -> Result<()>
where
    F: FnMut(&str, &str),
{
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            load_json_files_recursive(base, &path, namespace, handler)?;
        } else if path.extension().map(|e| e == "json").unwrap_or(false) {
            let relative = path
                .strip_prefix(base)
                .unwrap()
                .with_extension("")
                .to_string_lossy()
                .replace('\\', "/");

            let contents = std::fs::read_to_string(&path)?;
            handler(&relative, &contents);
        }
    }
    Ok(())
}

/// Load texture files recursively from a directory.
fn load_texture_files_recursive<F>(
    base: &Path,
    dir: &Path,
    namespace: &str,
    handler: &mut F,
) -> Result<()>
where
    F: FnMut(&str, &[u8]),
{
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            load_texture_files_recursive(base, &path, namespace, handler)?;
        } else if path.extension().map(|e| e == "png").unwrap_or(false) {
            let relative = path
                .strip_prefix(base)
                .unwrap()
                .with_extension("")
                .to_string_lossy()
                .replace('\\', "/");

            let data = std::fs::read(&path)?;
            handler(&relative, &data);
        }
    }
    Ok(())
}

/// Load .png.mcmeta files recursively from a directory.
fn load_mcmeta_files_recursive<F>(
    base: &Path,
    dir: &Path,
    handler: &mut F,
) -> Result<()>
where
    F: FnMut(&str, &str),
{
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            load_mcmeta_files_recursive(base, &path, handler)?;
        } else {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.ends_with(".png.mcmeta") {
                // Strip ".png.mcmeta" to get the texture path relative to base
                let relative = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                let texture_path = relative.trim_end_matches(".png.mcmeta");
                let contents = std::fs::read_to_string(&path)?;
                handler(texture_path, &contents);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_asset_path() {
        assert_eq!(
            parse_asset_path("assets/minecraft/blockstates/stone.json"),
            Some(("minecraft", "blockstates", "stone.json"))
        );
        assert_eq!(
            parse_asset_path("assets/minecraft/models/block/stone.json"),
            Some(("minecraft", "models", "block/stone.json"))
        );
        assert_eq!(
            parse_asset_path("assets/mymod/textures/block/custom.png"),
            Some(("mymod", "textures", "block/custom.png"))
        );
        assert_eq!(parse_asset_path("pack.mcmeta"), None);
        assert_eq!(parse_asset_path("data/minecraft/recipes/test.json"), None);
    }
}
