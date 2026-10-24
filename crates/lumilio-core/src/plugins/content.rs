use std::time::Duration;

use lumilio_plugin_api::content::{Capabilities, SearchPage, SearchQuery};
use lumilio_plugin_api::{ContentSource, HostContext, PluginError};

use super::PluginHost;

/// Content sources read the network; a search or a long version list takes
/// several requests. All of one call's requests share this budget.
const CONTENT_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginContentSource {
    pub plugin: String,
    pub name: String,
    pub capabilities: Capabilities,
}

impl PluginHost {
    /// Only active content sources contribute choices, in the order the
    /// application listed them (the first is the default). Plugin methods,
    /// including capability declarations, run inside the worker boundary.
    pub async fn content_sources(&self) -> Vec<PluginContentSource> {
        let locale = self.locale_tag();
        let mut sources = Vec::new();
        for info in self.list().await {
            let capabilities = self
                .call_content(&info.manifest.id, |source, _| Ok(source.capabilities()))
                .await;
            if let Some(capabilities) = capabilities {
                self.content_ids
                    .lock()
                    .expect("plugin content state lock was poisoned")
                    .insert(info.manifest.id.clone());
                let at = self.listed_at(&info.manifest.id).await;
                sources.push((
                    at,
                    PluginContentSource {
                        plugin: info.manifest.id,
                        name: info.manifest.name.get(&locale).to_owned(),
                        capabilities,
                    },
                ));
            }
        }
        sources.sort_by_key(|(at, _)| *at);
        sources.into_iter().map(|(_, source)| source).collect()
    }

    /// A content source that was answering and has since stopped, with why.
    /// Only for telling "stopped" from "none"; a disabled source is neither.
    pub async fn stopped_content_source(&self) -> Option<(String, String)> {
        let locale = self.locale_tag();
        let known = self
            .content_ids
            .lock()
            .expect("plugin content state lock was poisoned")
            .clone();
        self.list()
            .await
            .into_iter()
            .find_map(|info| match info.status {
                super::PluginStatus::Failed { message } if known.contains(&info.manifest.id) => {
                    Some((info.manifest.name.get(&locale).to_owned(), message))
                }
                _ => None,
            })
    }

    /// Dispatch for project/version/file/dependency/recognition operations,
    /// with the content budget. Missing, disabled or failed sources yield no
    /// contribution.
    pub async fn call_content<R, F>(&self, id: &str, call: F) -> Option<R>
    where
        R: Send + 'static,
        F: FnOnce(&dyn ContentSource, &dyn HostContext) -> Result<R, PluginError> + Send + 'static,
    {
        self.call_in_with_timeout(id, None, CONTENT_TIMEOUT, move |plugin, ctx| {
            plugin
                .content_source()
                .map(|source| call(source, ctx))
                .transpose()
        })
        .await
        .flatten()
    }

    /// Search, within the same budget as every content call.
    pub async fn search_content(&self, id: &str, query: SearchQuery) -> Option<SearchPage> {
        self.call_in_with_timeout(id, None, CONTENT_TIMEOUT, move |plugin, ctx| {
            plugin
                .content_source()
                .map(|source| source.search(ctx, &query))
                .transpose()
        })
        .await
        .flatten()
    }
}
