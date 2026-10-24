//! The Discover search request: what the page asks a content source for.
//!
//! The selection model follows Modrinth App's `packages/ui/src/utils/search.ts`
//! (`newFilters`), GPL-3.0-only; see ADR 0022. Turning it into a source's own
//! protocol is that source plugin's job.

use super::kinds::{ProjectKind, SortIndex};

/// Whether a picked option must be present or must be absent.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Stance {
    #[default]
    Include,
    Exclude,
}

/// One filter option the person picked.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pick {
    pub name: String,
    pub stance: Stance,
    /// Included picks of an "any of" group match when any one does (loaders,
    /// resolutions); otherwise every included pick has to match.
    pub any: bool,
}

impl Pick {
    #[must_use]
    pub fn include(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            stance: Stance::Include,
            any: false,
        }
    }

    #[must_use]
    pub fn exclude(name: impl Into<String>) -> Self {
        Self {
            stance: Stance::Exclude,
            ..Self::include(name)
        }
    }

    #[must_use]
    pub const fn any_of(mut self) -> Self {
        self.any = true;
        self
    }
}

#[derive(Clone, Debug)]
pub struct SearchQuery {
    pub text: String,
    pub kind: ProjectKind,
    pub sort: SortIndex,
    /// Zero-based.
    pub page: u32,
    pub page_size: u32,
    /// Game versions; a project matches when it supports any of them.
    pub game_versions: Vec<String>,
    /// Loader names (`fabric`, `forge`, …). Only mods, modpacks and shaders
    /// have loaders worth filtering by; other kinds ignore them. Included
    /// loaders always match as "any of".
    pub loaders: Vec<Pick>,
    pub categories: Vec<Pick>,
    /// Environment: either side asked for. Both asked narrows to projects that
    /// run on both.
    pub client: bool,
    pub server: bool,
    pub open_source: Option<Stance>,
    /// Projects to leave out (what the game already has).
    pub hidden_projects: Vec<String>,
    /// Disclosure kinds to leave out (`epilepsy_triggers`, …).
    pub excluded_disclosures: Vec<String>,
    /// Other project types a project must not also be (`plugin`, `datapack`).
    pub excluded_types: Vec<String>,
}

impl SearchQuery {
    #[must_use]
    pub fn new(kind: ProjectKind) -> Self {
        Self {
            text: String::new(),
            kind,
            sort: SortIndex::default(),
            page: 0,
            page_size: 20,
            game_versions: Vec::new(),
            loaders: Vec::new(),
            categories: Vec::new(),
            client: false,
            server: false,
            open_source: None,
            hidden_projects: Vec::new(),
            excluded_disclosures: Vec::new(),
            excluded_types: Vec::new(),
        }
    }

    /// Whether this kind has loaders to filter by.
    #[must_use]
    pub const fn has_loaders(&self) -> bool {
        matches!(
            self.kind,
            ProjectKind::Mod | ProjectKind::Modpack | ProjectKind::Shader
        )
    }

    /// Only mods and modpacks say which side they run on.
    #[must_use]
    pub const fn has_environment(&self) -> bool {
        matches!(self.kind, ProjectKind::Mod | ProjectKind::Modpack)
    }
}
