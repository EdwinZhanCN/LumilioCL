use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use super::{cache, document, protocol};
use crate::discover::{Catalog, TextFormat};
use crate::transfer::Transport;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationProvider {
    #[default]
    MicrosoftEdge,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct TranslationPreferences {
    pub enabled: bool,
    pub provider: TranslationProvider,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranslationError {
    Disabled,
    Network,
    Http(u16),
    Protocol,
    TooLarge,
    Format,
    Storage,
}

impl std::fmt::Display for TranslationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "translation: {self:?}")
    }
}
impl std::error::Error for TranslationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslatedDocument {
    pub text: String,
    pub partial: bool,
}

pub(crate) struct TranslationService {
    pub(super) cancel: std::sync::Mutex<crate::CancellationToken>,
    pub(super) cache: cache::Cache,
    /// Serializes translation jobs without holding launcher settings; coalesces
    /// repeated requests through the cache and bounds outbound concurrency.
    pub(super) gate: Mutex<()>,
}

impl TranslationService {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            cancel: std::sync::Mutex::new(crate::CancellationToken::new()),
            cache: cache::Cache::new(root.join("cache/translations.sqlite")),
            gate: Mutex::new(()),
        }
    }

    pub(crate) fn invalidate(&self) {
        let mut cancel = self
            .cancel
            .lock()
            .expect("translation cache lock was poisoned");
        cancel.cancel();
        *cancel = crate::CancellationToken::new();
        self.cache.invalidate();
    }

    pub(crate) async fn clear(&self) -> Result<(), TranslationError> {
        self.invalidate();
        self.cache.clear().await
    }

    pub(crate) async fn body<T: Transport>(
        &self,
        transport: &T,
        source: &str,
        project: &str,
        original: &str,
        format: TextFormat,
    ) -> Result<TranslatedDocument, TranslationError> {
        let cancel = self
            .cancel
            .lock()
            .expect("translation cache lock was poisoned")
            .clone();
        tokio::select! { () = cancel.cancelled() => Err(TranslationError::Disabled), result = self.body_inner(transport, source, project, original, format) => result }
    }

    pub(super) async fn body_inner<T: Transport>(
        &self,
        transport: &T,
        source: &str,
        project: &str,
        original: &str,
        format: TextFormat,
    ) -> Result<TranslatedDocument, TranslationError> {
        if original.len() > 2 * 1024 * 1024 {
            return Err(TranslationError::TooLarge);
        }
        let _guard = self.gate.lock().await;
        let generation = self.cache.generation();
        let field = match format {
            TextFormat::Markdown => "body",
            TextFormat::Html => "body-html",
        };
        let identity = key(source, project, field, "edge", original);
        if let Some(text) = self
            .cache
            .get(identity.clone(), crate::service::now())
            .await
        {
            return Ok(TranslatedDocument {
                text,
                partial: false,
            });
        }
        let parts = document::Document::parse(original, format)?.texts();
        let mut translated = Vec::with_capacity(parts.len());
        let mut partial = false;
        for batch in document::batches(&parts) {
            let mut outputs = Vec::new();
            let mut missing = Vec::new();
            for (index, text) in batch.iter().enumerate() {
                let part_key = key(source, project, "segment-v1", "edge", text);
                match self.cache.get(part_key, crate::service::now()).await {
                    Some(saved) => outputs.push(Some(saved)),
                    None => {
                        outputs.push(None);
                        missing.push(index);
                    }
                }
            }
            if !missing.is_empty() {
                let requested: Vec<String> = missing.iter().map(|&i| batch[i].clone()).collect();
                match protocol::edge(transport, &requested).await {
                    Ok(found) => {
                        for (index, text) in missing.into_iter().zip(found) {
                            self.cache
                                .put(
                                    key(source, project, "segment-v1", "edge", &batch[index]),
                                    text.clone(),
                                    crate::service::now(),
                                    generation,
                                )
                                .await;
                            outputs[index] = Some(text);
                        }
                    }
                    Err(error)
                        if translated.is_empty()
                            && outputs.iter().all(Option::is_none)
                            && batch.len() == parts.len() =>
                    {
                        return Err(error);
                    }
                    Err(_) => partial = true,
                }
            }
            for (original, output) in batch.iter().zip(outputs) {
                translated.push(output.unwrap_or_else(|| {
                    partial = true;
                    original.clone()
                }));
            }
            if translated.iter().map(String::len).sum::<usize>() > 8 * 1024 * 1024 {
                return Err(TranslationError::TooLarge);
            }
        }
        let text = document::Document::parse(original, format)?.replace(&translated)?;
        if !partial {
            self.cache
                .put(identity, text.clone(), crate::service::now(), generation)
                .await;
        }
        Ok(TranslatedDocument { text, partial })
    }

    /// `catalog` is whose IDs the source speaks; MCIM and our fallback only
    /// know those two, so a source that declares none gets no summaries.
    pub(crate) async fn summaries<T: Transport>(
        &self,
        transport: &T,
        catalog: Option<Catalog>,
        source: &str,
        items: &[(String, String)],
    ) -> BTreeMap<String, String> {
        let cancel = self
            .cancel
            .lock()
            .expect("translation cache lock was poisoned")
            .clone();
        tokio::select! { () = cancel.cancelled() => BTreeMap::new(), result = self.summaries_inner(transport, catalog, source, items) => result }
    }

    pub(super) async fn summaries_inner<T: Transport>(
        &self,
        transport: &T,
        catalog: Option<Catalog>,
        source: &str,
        items: &[(String, String)],
    ) -> BTreeMap<String, String> {
        let platform = match catalog {
            Some(Catalog::Modrinth) => "modrinth",
            Some(Catalog::CurseForge) => "curseforge",
            None => return BTreeMap::new(),
        };
        let _guard = self.gate.lock().await;
        let generation = self.cache.generation();
        let mut result = BTreeMap::new();
        // What no service has answered yet; each service only hears of these.
        let mut unresolved = items.to_vec();
        for (provider, base) in protocol::SUMMARY_SERVICES {
            let mut ask = Vec::new();
            let mut next = Vec::new();
            for (id, original) in unresolved {
                match self
                    .cache
                    .get(
                        key(source, &id, "summary", provider, &original),
                        crate::service::now(),
                    )
                    .await
                {
                    Some(text) if !text.is_empty() => {
                        result.insert(id, text);
                    }
                    // Lately known to have nothing here.
                    Some(_) => next.push((id, original)),
                    None => ask.push((id, original)),
                }
            }
            for batch in ask.chunks(50) {
                let ids: Vec<String> = batch.iter().map(|(id, _)| id.clone()).collect();
                let Ok(found) = protocol::summaries(transport, base, platform, &ids).await else {
                    next.extend_from_slice(batch);
                    continue;
                };
                for (id, original) in batch {
                    if !found.contains_key(id) {
                        self.cache
                            .put_for(
                                key(source, id, "summary", provider, original),
                                String::new(),
                                crate::service::now(),
                                generation,
                                10 * 60,
                            )
                            .await;
                    }
                    if let Some((remote_original, translated)) = found.get(id)
                        && remote_original == original
                        && !translated.trim().is_empty()
                        && document::safe_summary(original, translated)
                    {
                        self.cache
                            .put(
                                key(source, id, "summary", provider, original),
                                translated.clone(),
                                crate::service::now(),
                                generation,
                            )
                            .await;
                        result.insert(id.clone(), translated.clone());
                    } else {
                        next.push((id.clone(), original.clone()));
                    }
                }
            }
            unresolved = next;
            if unresolved.is_empty() {
                break;
            }
        }
        result
    }
}

pub(super) fn key(
    source: &str,
    project: &str,
    field: &str,
    provider: &str,
    original: &str,
) -> String {
    let encoded = serde_json::to_vec(&(
        source,
        project,
        field,
        provider,
        "zh-Hans",
        "format-v1",
        original,
    ))
    .expect("string tuple serializes");
    Sha256::digest(encoded)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) type SharedTranslation = Arc<TranslationService>;
