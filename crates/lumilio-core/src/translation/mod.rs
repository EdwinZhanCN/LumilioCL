//! Derived project text, independent of content plugins and presentation.
mod cache;
mod document;
mod protocol;
#[cfg(test)]
mod tests;

#[cfg(test)]
use std::collections::BTreeMap;
#[cfg(test)]
use std::sync::Arc;

#[cfg(test)]
use translation_service::key;
pub(crate) use translation_service::{SharedTranslation, TranslationService};
pub use translation_service::{
    TranslatedDocument, TranslationError, TranslationPreferences, TranslationProvider,
};

#[cfg(test)]
use crate::discover::{Catalog, TextFormat};
#[cfg(test)]
use crate::transfer::Transport;

mod translation_service;
