use lumilio_minecraft::{OFFICIAL_CATALOG_URL, VersionCatalog};

use crate::fetch::{FetchError, fetch_decoded};
use crate::transfer::{SourceChain, TransferEngine, Transport};

/// Fetches the official version catalog from the first source that answers
/// with a valid document.
pub async fn fetch_catalog<T: Transport>(
    engine: &TransferEngine<T>,
    chain: &SourceChain,
) -> Result<VersionCatalog, FetchError> {
    let sources = chain.candidates(OFFICIAL_CATALOG_URL);
    fetch_decoded(engine, &sources, |bytes| {
        let text = String::from_utf8(bytes).map_err(|error| error.utf8_error().to_string())?;
        VersionCatalog::decode_json(&text).map_err(|error| error.to_string())
    })
    .await
}

#[cfg(test)]
mod tests;
