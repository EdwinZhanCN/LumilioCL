//! The pre-baked bundle (D2): fetched on first need from the launcher's site
//! through the transport and mirror chain, checked against the SHA-256 its
//! index names, and kept under `Layout::model_cache()/bundle/<release>/`.

use std::fs;
use std::path::{Path, PathBuf};

use lumilio_block_assets::{
    BakedPack, BlockCatalog, BlockNames, Bundle, EntityBundle, EntityLayers,
};
use sha2::{Digest, Sha256};

use super::{BundleFile, BundleIndex, SchematicAssetsError};
use crate::activity::CancellationToken;
use crate::fetch::fetch_decoded;
use crate::transfer::{SourceChain, TransferEngine, TransferRequest, Transport};

/// The bundle's own release, independent of the launcher's version (D2).
/// A new bake is published under a new release; files under one never change.
pub const BUNDLE_RELEASE: &str = "1";
/// Where the bundle is published: `blocks.json` and the files it names.
pub const BUNDLE_URL: &str = "https://launcher.lumilio.org/block-assets/1/";
pub const INDEX_FILE: &str = "blocks.json";

/// Where the bundle is fetched from: [`BUNDLE_URL`], or the review hook
/// `LUMILIO_BLOCK_ASSETS_URL` (a folder address ending in `/`), so the
/// first download can be watched against a local server before a release
/// is published. Anything else is ignored.
pub(super) fn bundle_url(hook: Option<String>) -> String {
    hook.filter(|url| {
        (url.starts_with("https://") || url.starts_with("http://")) && url.ends_with('/')
    })
    .unwrap_or_else(|| BUNDLE_URL.to_owned())
}

#[derive(Clone, Debug)]
pub struct BundleStore {
    dir: PathBuf,
    url: String,
}

impl BundleStore {
    #[must_use]
    pub fn new(model_cache: &Path) -> Self {
        Self {
            dir: model_cache.join("bundle").join(BUNDLE_RELEASE),
            url: bundle_url(std::env::var("LUMILIO_BLOCK_ASSETS_URL").ok()),
        }
    }

    /// The same store fetching from `url` (a folder address ending in `/`).
    #[cfg(test)]
    #[must_use]
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    #[must_use]
    pub fn path_of(&self, file: &BundleFile) -> PathBuf {
        self.dir.join(&file.name)
    }

    /// The index fetched before, if any. Blocking.
    #[must_use]
    pub fn cached_index(&self) -> Option<BundleIndex> {
        let bytes = fs::read(self.dir.join(INDEX_FILE)).ok()?;
        BundleIndex::parse(&bytes).ok()
    }

    /// Fetches the index and keeps it: a release's index never changes.
    pub async fn fetch_index<T: Transport + ?Sized>(
        &self,
        transport: &T,
        chain: &SourceChain,
    ) -> Result<BundleIndex, SchematicAssetsError> {
        let sources = chain.candidates(&format!("{}{INDEX_FILE}", self.url));
        let (index, bytes) = fetch_decoded(transport, &sources, |bytes| {
            BundleIndex::parse(&bytes).map(|index| (index, bytes))
        })
        .await
        .map_err(|error| SchematicAssetsError::Download(error.to_string()))?;
        let dir = self.dir.clone();
        let _ = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            fs::create_dir_all(&dir)?;
            let partial = dir.join(format!("{INDEX_FILE}.partial"));
            fs::write(&partial, bytes)?;
            fs::rename(partial, dir.join(INDEX_FILE))
        })
        .await;
        Ok(index)
    }

    /// Whether `file` is here at its full size (its hash is checked on read).
    #[must_use]
    pub fn has(&self, file: &BundleFile) -> bool {
        fs::metadata(self.path_of(file)).is_ok_and(|meta| file.size == 0 || meta.len() == file.size)
    }

    /// Downloads `file` through `engine` (whose events the caller may show).
    /// A file that fails its check is not kept, nor is a damaged one that
    /// was here before.
    pub async fn download<T: Transport + Clone>(
        &self,
        engine: &TransferEngine<T>,
        chain: &SourceChain,
        file: &BundleFile,
        cancel: CancellationToken,
    ) -> Result<PathBuf, SchematicAssetsError> {
        let destination = self.path_of(file);
        let mut request = TransferRequest::new(
            format!("block-assets:{}", file.name),
            chain.candidates(&format!("{}{}", self.url, file.name)),
            &destination,
        )
        .and_then(|request| request.expect_sha256(&file.sha256))
        .map_err(|error| SchematicAssetsError::Download(error.to_string()))?;
        if file.size > 0 {
            request = request.expect_size(file.size);
        }
        tokio::fs::create_dir_all(&self.dir).await?;
        match engine.transfer(request, cancel).await {
            Ok(_) => Ok(destination),
            Err(error) => {
                let _ = tokio::fs::remove_file(&destination).await;
                Err(SchematicAssetsError::Download(error.to_string()))
            }
        }
    }

    /// `version`'s pack out of the downloaded `file`, after checking the
    /// whole file against its SHA-256. A file that does not match is
    /// deleted. Blocking.
    pub fn slice(
        &self,
        file: &BundleFile,
        version: &str,
    ) -> Result<BakedPack, SchematicAssetsError> {
        Ok(Bundle::read(&self.checked(file)?)?.pack(version)?)
    }

    /// The block states each of `versions` knows, out of the downloaded
    /// `file` (checked like [`Self::slice`]; images are not decoded).
    /// Blocking.
    pub fn catalogs(
        &self,
        file: &BundleFile,
        versions: &[&str],
    ) -> Result<Vec<BlockCatalog>, SchematicAssetsError> {
        let bundle = Bundle::read(&self.checked(file)?)?;
        versions
            .iter()
            .map(|version| Ok(bundle.catalog(version)?))
            .collect()
    }

    /// Fetches a small `file` (the names) without an Activity task, as the
    /// index is fetched, and keeps it once it matches its SHA-256.
    pub async fn fetch_small<T: Transport + ?Sized>(
        &self,
        transport: &T,
        chain: &SourceChain,
        file: &BundleFile,
    ) -> Result<(), SchematicAssetsError> {
        let sources = chain.candidates(&format!("{}{}", self.url, file.name));
        let expected = file.sha256.clone();
        let bytes = fetch_decoded(transport, &sources, |bytes| {
            if sha256_hex(&bytes).eq_ignore_ascii_case(&expected) {
                Ok(bytes)
            } else {
                Err("the file does not match the SHA-256 its index names")
            }
        })
        .await
        .map_err(|error| SchematicAssetsError::Download(error.to_string()))?;
        let (dir, name) = (self.dir.clone(), file.name.clone());
        tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            fs::create_dir_all(&dir)?;
            let partial = dir.join(format!("{name}.partial"));
            fs::write(&partial, bytes)?;
            fs::rename(partial, dir.join(name))
        })
        .await
        .map_err(|error| SchematicAssetsError::Download(error.to_string()))??;
        Ok(())
    }

    /// The names out of the downloaded `names.dat.z`, checked like
    /// [`Self::slice`]. Blocking.
    pub fn names(&self, file: &BundleFile) -> Result<BlockNames, SchematicAssetsError> {
        Ok(BlockNames::read(&self.checked(file)?)?)
    }

    /// `version`'s entity layers out of the downloaded `entities.dat.z`,
    /// checked like [`Self::slice`]. Blocking.
    pub fn entity_layers(
        &self,
        file: &BundleFile,
        version: &str,
    ) -> Result<EntityLayers, SchematicAssetsError> {
        Ok(EntityBundle::read(&self.checked(file)?)?.layers_of(version))
    }

    /// The bytes of the downloaded `file` when they match its SHA-256; a
    /// file that does not is deleted.
    fn checked(&self, file: &BundleFile) -> Result<Vec<u8>, SchematicAssetsError> {
        let path = self.path_of(file);
        let bytes = fs::read(&path)?;
        if !sha256_hex(&bytes).eq_ignore_ascii_case(&file.sha256) {
            let _ = fs::remove_file(&path);
            return Err(SchematicAssetsError::Mismatch {
                file: file.name.clone(),
            });
        }
        Ok(bytes)
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
