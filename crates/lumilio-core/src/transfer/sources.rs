use super::request::TransferError;
use std::collections::BTreeSet;
use std::fmt::Debug;
use std::sync::Arc;

pub trait SourceProvider: Send + Sync + 'static {
    fn candidates(&self, original: &str) -> Vec<String>;

    fn maximum_concurrency(&self) -> usize {
        16
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OfficialSource;

impl SourceProvider for OfficialSource {
    fn candidates(&self, original: &str) -> Vec<String> {
        vec![original.to_owned()]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrefixMirror {
    pub(super) official_prefix: String,
    pub(super) mirror_prefix: String,
    pub(super) concurrency: usize,
}

impl PrefixMirror {
    pub fn new(
        official_prefix: impl Into<String>,
        mirror_prefix: impl Into<String>,
        concurrency: usize,
    ) -> Result<Self, TransferError> {
        let official_prefix = official_prefix.into();
        let mirror_prefix = mirror_prefix.into();
        if official_prefix.is_empty() || mirror_prefix.is_empty() {
            return Err(TransferError::InvalidRequest(
                "mirror prefixes must not be empty".to_owned(),
            ));
        }
        if concurrency == 0 {
            return Err(TransferError::InvalidRequest(
                "mirror concurrency must be positive".to_owned(),
            ));
        }
        Ok(Self {
            official_prefix,
            mirror_prefix,
            concurrency,
        })
    }
}

impl SourceProvider for PrefixMirror {
    fn candidates(&self, original: &str) -> Vec<String> {
        original
            .strip_prefix(&self.official_prefix)
            .map(|suffix| format!("{}{}", self.mirror_prefix, suffix))
            .into_iter()
            .collect()
    }

    fn maximum_concurrency(&self) -> usize {
        self.concurrency
    }
}

#[derive(Clone)]
pub struct SourceChain {
    pub(super) providers: Vec<Arc<dyn SourceProvider>>,
}

impl SourceChain {
    pub fn new(
        providers: impl IntoIterator<Item = Arc<dyn SourceProvider>>,
    ) -> Result<Self, TransferError> {
        let providers = providers.into_iter().collect::<Vec<_>>();
        if providers.is_empty() {
            return Err(TransferError::InvalidRequest(
                "at least one source provider is required".to_owned(),
            ));
        }
        Ok(Self { providers })
    }

    #[must_use]
    pub fn candidates(&self, original: &str) -> Vec<String> {
        let mut seen = BTreeSet::new();
        self.providers
            .iter()
            .flat_map(|provider| provider.candidates(original))
            .filter(|source| !source.trim().is_empty())
            .filter(|source| seen.insert(source.clone()))
            .collect()
    }

    #[must_use]
    pub fn preferred_concurrency(&self) -> usize {
        self.providers[0].maximum_concurrency().max(1)
    }
}
