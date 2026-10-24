use lumilio_minecraft::CatalogEntry;

use crate::error::ServiceError;
use crate::fetch::fetch_decoded;
use crate::instances::Loader;
use crate::network::Network;
use crate::transfer::Transport;
use crate::{forge_meta, loader};

/// Every game version the catalog lists, newest first, for choosing one
/// (IA P-VERSION). Read fresh each time; a failure is reported, never
/// replaced by a guess.
pub(crate) async fn game_versions<T: Transport + Clone>(
    network: &Network<T>,
) -> Result<Vec<CatalogEntry>, ServiceError> {
    let chain = network.chain().await?;
    let catalog = crate::minecraft::fetch_catalog(&network.fetcher(), &chain)
        .await
        .map_err(|error| ServiceError::Remote(error.to_string()))?;
    Ok(catalog.entries().to_vec())
}

/// The versions of `loader` available for `game_version`, newest first
/// (IA P-LOADER-VERSION). Vanilla has none.
pub(crate) async fn loader_versions<T: Transport + Clone>(
    network: &Network<T>,
    loader: Loader,
    game_version: &str,
) -> Result<Vec<loader::LoaderVersion>, ServiceError> {
    let chain = network.chain().await?;
    let remote = |error: crate::fetch::FetchError| ServiceError::Remote(error.to_string());
    let decode = |error: loader::LoaderError| ServiceError::Remote(error.to_string());
    match loader {
        Loader::Vanilla => Ok(Vec::new()),
        Loader::Fabric | Loader::Quilt => {
            let url = loader::versions_url(loader, game_version)
                .ok_or_else(|| ServiceError::Remote("no loader source".to_owned()))?;
            loader::fetch_versions(&network.fetcher(), &chain.candidates(&url))
                .await
                .map_err(decode)
        }
        Loader::Forge => {
            let metadata = fetch_decoded(
                &network.fetcher(),
                &chain.candidates(forge_meta::FORGE_METADATA_URL),
                |bytes| {
                    forge_meta::forge_versions(&bytes, None, game_version)?;
                    Ok::<_, loader::LoaderError>(bytes)
                },
            )
            .await
            .map_err(remote)?;
            // Promotions only mark the recommended build; the list stands without them.
            let promotions = fetch_decoded(
                &network.fetcher(),
                &chain.candidates(forge_meta::FORGE_PROMOTIONS_URL),
                |bytes| {
                    forge_meta::forge_versions(&metadata, Some(&bytes), game_version)?;
                    Ok::<_, loader::LoaderError>(bytes)
                },
            )
            .await
            .ok();
            forge_meta::forge_versions(&metadata, promotions.as_deref(), game_version)
                .map_err(decode)
        }
        Loader::NeoForge => {
            let modern = fetch_decoded(
                &network.fetcher(),
                &chain.candidates(forge_meta::NEOFORGE_VERSIONS_URL),
                |bytes| {
                    forge_meta::neoforge_versions(&bytes, None, game_version)?;
                    Ok::<_, loader::LoaderError>(bytes)
                },
            )
            .await
            .map_err(remote)?;
            let legacy = if game_version == forge_meta::NEOFORGE_LEGACY_GAME {
                Some(
                    fetch_decoded(
                        &network.fetcher(),
                        &chain.candidates(forge_meta::NEOFORGE_LEGACY_VERSIONS_URL),
                        |bytes| {
                            forge_meta::neoforge_versions(&modern, Some(&bytes), game_version)?;
                            Ok::<_, loader::LoaderError>(bytes)
                        },
                    )
                    .await
                    .map_err(remote)?,
                )
            } else {
                None
            };
            forge_meta::neoforge_versions(&modern, legacy.as_deref(), game_version).map_err(decode)
        }
    }
}
