use super::assets::AssetIndex;
use super::installer::InstallError;
use super::native::unique_sibling;
use super::plan::InstallationPlan;
use crate::activity::CancellationToken;
use std::io;
use std::path::{Component, Path, PathBuf};
use tokio::fs;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

pub(super) async fn publish_asset_views(
    plan: &InstallationPlan,
    index: &AssetIndex,
    cancellation: &CancellationToken,
) -> Result<usize, InstallError> {
    let Some(catalog_id) = plan.asset_catalog_id.as_deref() else {
        return Ok(0);
    };
    let mut published = 0_usize;
    for (logical_name, object) in index.objects() {
        ensure_not_cancelled(cancellation)?;
        let relative = validate_protocol_path(logical_name).map_err(InstallError::Plan)?;
        let object_path = plan
            .directories
            .assets()
            .join("objects")
            .join(object.relative_path());
        if index.uses_virtual_layout() {
            let destination = plan
                .directories
                .assets()
                .join("virtual")
                .join(catalog_id)
                .join(&relative);
            publish_file_view(&object_path, &destination).await?;
            published += 1;
        }
        if index.maps_to_resources() {
            let destination = plan.directories.game().join("resources").join(&relative);
            publish_file_view(&object_path, &destination).await?;
            published += 1;
        }
    }
    Ok(published)
}

pub(super) async fn publish_file_view(
    source: &Path,
    destination: &Path,
) -> Result<(), InstallError> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .await
        .map_err(|error| install_fs_error("create asset view directory", parent, error))?;
    let temporary = unique_sibling(destination, "asset");
    match fs::hard_link(source, &temporary).await {
        Ok(()) => {}
        Err(_) => {
            fs::copy(source, &temporary)
                .await
                .map_err(|error| install_fs_error("copy asset view", source, error))?;
        }
    }
    replace_path(&temporary, destination).await
}

pub(super) async fn atomic_write(destination: &Path, bytes: &[u8]) -> Result<(), InstallError> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .await
        .map_err(|error| install_fs_error("create manifest directory", parent, error))?;
    let temporary = unique_sibling(destination, "manifest");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .await
        .map_err(|error| install_fs_error("create manifest staging file", &temporary, error))?;
    if let Err(error) = file.write_all(bytes).await {
        let _ = fs::remove_file(&temporary).await;
        return Err(install_fs_error(
            "write manifest staging file",
            &temporary,
            error,
        ));
    }
    if let Err(error) = file.flush().await {
        let _ = fs::remove_file(&temporary).await;
        return Err(install_fs_error(
            "flush manifest staging file",
            &temporary,
            error,
        ));
    }
    if let Err(error) = file.sync_all().await {
        let _ = fs::remove_file(&temporary).await;
        return Err(install_fs_error(
            "sync manifest staging file",
            &temporary,
            error,
        ));
    }
    drop(file);
    replace_path(&temporary, destination).await
}

pub(super) async fn replace_path(temporary: &Path, destination: &Path) -> Result<(), InstallError> {
    let backup = unique_sibling(destination, "backup");
    let had_destination = fs::symlink_metadata(destination).await.is_ok();
    if had_destination {
        fs::rename(destination, &backup)
            .await
            .map_err(|error| install_fs_error("stage previous destination", destination, error))?;
    }
    if let Err(error) = fs::rename(temporary, destination).await {
        if had_destination {
            let _ = fs::rename(&backup, destination).await;
        }
        let _ = fs::remove_file(temporary).await;
        return Err(install_fs_error(
            "publish staged destination",
            destination,
            error,
        ));
    }
    if had_destination {
        remove_async_path(&backup).await?;
    }
    Ok(())
}

pub(super) async fn remove_async_path(path: &Path) -> Result<(), InstallError> {
    let metadata = match fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(install_fs_error("inspect old destination", path, error)),
    };
    if metadata.is_dir() {
        fs::remove_dir_all(path)
            .await
            .map_err(|error| install_fs_error("remove old directory", path, error))
    } else {
        fs::remove_file(path)
            .await
            .map_err(|error| install_fs_error("remove old file", path, error))
    }
}

pub(super) fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<(), InstallError> {
    if cancellation.is_cancelled() {
        Err(InstallError::Cancelled)
    } else {
        Ok(())
    }
}

pub(super) fn install_fs_error(
    operation: &'static str,
    path: &Path,
    error: io::Error,
) -> InstallError {
    InstallError::FileSystem {
        operation,
        path: path.to_owned(),
        message: error.to_string(),
    }
}

pub(super) fn validate_sha1(hash: &str) -> Result<(), ()> {
    if hash.len() == 40 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(())
    }
}

pub(super) fn validate_single_component(value: &str) -> Result<PathBuf, String> {
    let path = validate_protocol_path(value)?;
    if path.components().count() == 1 {
        Ok(path)
    } else {
        Err("must contain exactly one path component".to_owned())
    }
}

pub(super) fn validate_protocol_path(raw: &str) -> Result<PathBuf, String> {
    if raw.is_empty() {
        return Err("path is empty".to_owned());
    }
    if raw.contains('\0') {
        return Err("path contains a NUL byte".to_owned());
    }
    if raw.contains('\\') {
        return Err("backslash path aliases are not accepted".to_owned());
    }
    if raw.contains(':') {
        return Err("platform path prefixes are not accepted".to_owned());
    }
    let path = Path::new(raw);
    if path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        Ok(path.to_owned())
    } else {
        Err("path must contain only normal relative components".to_owned())
    }
}
