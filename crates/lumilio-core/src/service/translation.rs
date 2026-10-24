use std::collections::BTreeMap;

use super::LauncherService;
use crate::discover::TextFormat;
use crate::transfer::Transport;
use crate::{TranslatedDocument, TranslationError};

impl<T: Transport + Clone> LauncherService<T> {
    /// A project body in Simplified Chinese, from the chosen provider. The
    /// result is HTML whatever `format` the source wrote. Turning translation
    /// off (or changing its settings) while this runs gives `Disabled`.
    pub async fn translate_body(
        &self,
        source: &str,
        project: &str,
        original: &str,
        format: TextFormat,
    ) -> Result<TranslatedDocument, TranslationError> {
        let preferences = self.translation_preferences().await;
        if !preferences.enabled {
            return Err(TranslationError::Disabled);
        }
        let result = self
            .translation
            .body(self.network.transport(), source, project, original, format)
            .await;
        if self.translation_preferences().await != preferences {
            return Err(TranslationError::Disabled);
        }
        result
    }

    /// MCIM's Chinese summaries for `(project id, summary)` pairs from one
    /// source, by project id. Missing, stale or failed ones are left out, so
    /// the caller keeps the original; a source whose IDs are neither
    /// Modrinth's nor CurseForge's gets nothing.
    pub async fn translate_summaries(
        &self,
        source: &str,
        items: &[(String, String)],
    ) -> BTreeMap<String, String> {
        if !self.translation_preferences().await.enabled {
            return BTreeMap::new();
        }
        let Ok(catalog) = self
            .content_client(source)
            .await
            .map(|client| client.catalog())
        else {
            return BTreeMap::new();
        };
        let result = self
            .translation
            .summaries(self.network.transport(), catalog, source, items)
            .await;
        if !self.translation_preferences().await.enabled {
            return BTreeMap::new();
        }
        result
    }

    pub async fn clear_translation_cache(&self) -> Result<(), TranslationError> {
        self.translation.clear().await
    }

    async fn translation_preferences(&self) -> crate::TranslationPreferences {
        self.settings
            .lock()
            .await
            .get()
            .preferences
            .translation
            .clone()
    }
}
