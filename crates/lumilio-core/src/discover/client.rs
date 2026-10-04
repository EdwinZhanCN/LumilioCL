use super::API_BASE;
use super::error::DiscoverError;
use super::kinds::loader_tag;
use super::project::{Project, decode_project};
use super::search::{SearchPage, SearchQuery, decode_search};
use super::tags::{
    CategoryTag, GameVersionTag, ProjectSummary, decode_categories, decode_game_versions,
    decode_owner, decode_project_summaries, decode_team_authors, encoded_list,
};
use super::versions::{Version, decode_version_map, decode_versions};
use crate::fetch::{fetch_document, post_document};
use crate::instance::Loader;
use crate::transfer::{SourceChain, Transport};

/// A thin client over the public API.
pub struct ModrinthClient<T> {
    pub(super) transport: T,
    pub(super) base: String,
    /// Mirrors tried for read requests, as configured in settings.
    pub(super) sources: Option<SourceChain>,
}

impl<T: Transport> ModrinthClient<T> {
    #[must_use]
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            base: API_BASE.to_owned(),
            sources: None,
        }
    }

    /// Sends read requests through these mirrors before the official address.
    #[must_use]
    pub fn with_sources(mut self, sources: SourceChain) -> Self {
        self.sources = Some(sources);
        self
    }

    pub(super) fn candidates(&self, url: String) -> Vec<String> {
        match &self.sources {
            Some(chain) => chain.candidates(&url),
            None => vec![url],
        }
    }

    /// Points the client at another API root (tests, mirrors).
    #[must_use]
    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.base = base.into().trim_end_matches('/').to_owned();
        self
    }

    #[cfg(test)]
    pub(crate) fn transport_for_tests(&self) -> &T {
        &self.transport
    }

    pub(super) fn address(&self, url: String) -> String {
        url.replacen(API_BASE, &self.base, 1)
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<SearchPage, DiscoverError> {
        let bytes =
            fetch_document(&self.transport, &self.candidates(self.address(query.url()))).await?;
        decode_search(&bytes)
    }

    pub async fn project(&self, id_or_slug: &str) -> Result<Project, DiscoverError> {
        let url = format!("{}/v2/project/{id_or_slug}", self.base);
        decode_project(&fetch_document(&self.transport, &self.candidates(url)).await?)
    }

    /// Which Modrinth version each file (by SHA-1) belongs to. Files Modrinth
    /// does not know are absent from the answer.
    pub async fn identify(
        &self,
        sha1s: &[String],
    ) -> Result<std::collections::BTreeMap<String, Version>, DiscoverError> {
        if sha1s.is_empty() {
            return Ok(Default::default());
        }
        let body = serde_json::json!({ "hashes": sha1s, "algorithm": "sha1" });
        let url = format!("{}/v2/version_files", self.base);
        decode_version_map(
            &post_document(&self.transport, &url, body.to_string().into_bytes()).await?,
        )
    }

    /// The newest compatible version for each file (by SHA-1). Files with no
    /// compatible version, or unknown to Modrinth, are absent.
    pub async fn latest_for(
        &self,
        sha1s: &[String],
        loader: Loader,
        game_version: &str,
    ) -> Result<std::collections::BTreeMap<String, Version>, DiscoverError> {
        if sha1s.is_empty() {
            return Ok(Default::default());
        }
        let loaders: Vec<&str> = loader_tag(loader).into_iter().collect();
        let body = serde_json::json!({
            "hashes": sha1s,
            "algorithm": "sha1",
            "loaders": loaders,
            "game_versions": [game_version],
        });
        let url = format!("{}/v2/version_files/update", self.base);
        decode_version_map(
            &post_document(&self.transport, &url, body.to_string().into_bytes()).await?,
        )
    }

    pub async fn versions(&self, id_or_slug: &str) -> Result<Vec<Version>, DiscoverError> {
        let url = format!(
            "{}/v2/project/{id_or_slug}/version?include_changelog=false",
            self.base
        );
        decode_versions(&fetch_document(&self.transport, &self.candidates(url)).await?)
    }

    /// Like [`Self::versions`], with each version's changelog.
    pub async fn versions_with_changelog(
        &self,
        id_or_slug: &str,
    ) -> Result<Vec<Version>, DiscoverError> {
        let url = format!(
            "{}/v2/project/{id_or_slug}/version?include_changelog=true",
            self.base
        );
        decode_versions(&fetch_document(&self.transport, &self.candidates(url)).await?)
    }

    /// Every category tag, for all project types.
    pub async fn categories(&self) -> Result<Vec<CategoryTag>, DiscoverError> {
        let url = format!("{}/v2/tag/category", self.base);
        decode_categories(&fetch_document(&self.transport, &self.candidates(url)).await?)
    }

    /// Every game version Modrinth knows, newest first as published.
    pub async fn game_versions(&self) -> Result<Vec<GameVersionTag>, DiscoverError> {
        let url = format!("{}/v2/tag/game_version", self.base);
        decode_game_versions(&fetch_document(&self.transport, &self.candidates(url)).await?)
    }

    /// The project's owner, if the team lists one.
    pub async fn owner(&self, id_or_slug: &str) -> Result<Option<String>, DiscoverError> {
        let url = format!("{}/v2/project/{id_or_slug}/members", self.base);
        decode_owner(&fetch_document(&self.transport, &self.candidates(url)).await?)
    }

    /// Several projects at once, for labelling installed files.
    pub async fn project_summaries(
        &self,
        ids: &[String],
    ) -> Result<Vec<ProjectSummary>, DiscoverError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!("{}/v2/projects?ids={}", self.base, encoded_list(ids));
        decode_project_summaries(&fetch_document(&self.transport, &self.candidates(url)).await?)
    }

    /// Who each team's project belongs to, by team id.
    pub async fn team_authors(
        &self,
        team_ids: &[String],
    ) -> Result<std::collections::BTreeMap<String, String>, DiscoverError> {
        if team_ids.is_empty() {
            return Ok(Default::default());
        }
        let url = format!("{}/v2/teams?ids={}", self.base, encoded_list(team_ids));
        decode_team_authors(&fetch_document(&self.transport, &self.candidates(url)).await?)
    }
}
