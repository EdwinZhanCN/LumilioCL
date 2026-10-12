use super::LauncherService;
use crate::transfer::Transport;
use crate::{TranslatedDocument, TranslationError};
use std::collections::BTreeMap;

impl<T: Transport + Clone> LauncherService<T> {
    pub async fn translate_body(
        &self,
        source: &str,
        project: &str,
        original: &str,
    ) -> Result<TranslatedDocument, TranslationError> {
        let preferences = self
            .settings
            .lock()
            .await
            .get()
            .preferences
            .translation
            .clone();
        if !preferences.enabled {
            return Err(TranslationError::Disabled);
        }
        let result = self
            .translation
            .body(&self.transport, source, project, original)
            .await;
        if self.settings.lock().await.get().preferences.translation != preferences {
            return Err(TranslationError::Disabled);
        }
        result
    }

    pub async fn translate_summaries(
        &self,
        source: &str,
        items: &[(String, String)],
    ) -> BTreeMap<String, String> {
        if !self
            .settings
            .lock()
            .await
            .get()
            .preferences
            .translation
            .enabled
        {
            return BTreeMap::new();
        }
        let result = self
            .translation
            .summaries(&self.transport, source, items)
            .await;
        if !self
            .settings
            .lock()
            .await
            .get()
            .preferences
            .translation
            .enabled
        {
            return BTreeMap::new();
        }
        result
    }

    pub async fn clear_translation_cache(&self) -> Result<(), TranslationError> {
        self.translation.clear().await
    }
}
