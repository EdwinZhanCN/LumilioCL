//! What the Discover page is asking for, and the changes the page can make.
//!
//! The state follows Modrinth App's Browse page (`Browse.vue`,
//! `use-browse-search.ts`, GPL-3.0-only; ADR 0022): the person's own picks, the
//! filters a game provides (locked until released), and the toggles.

use super::filters::{FilterModel, advanced_options, is_type_exclusion};
use lumilio_core::{Loader, Pick, ProjectKind, SearchQuery, SortIndex, Stance};

/// Which list a loader or category pick belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PickGroup {
    Loader,
    Category,
}

/// A side of the connection the environment filter asks about.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
    Client,
    Server,
}

/// A filter a game provides, which the person may release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lock {
    Version,
    Loader,
}

/// One change to the query made on the page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiscoverChange {
    Kind(ProjectKind),
    Sort(SortIndex),
    PageSize(u32),
    /// Zero-based.
    Page(u32),
    ToggleVersion(String),
    ShowAllVersions(bool),
    /// Ask for the option (or drop it when it is already picked either way).
    Include(PickGroup, String),
    /// Leave the option out (or drop it when it is already left out).
    Exclude(PickGroup, String),
    ToggleSide(Side),
    /// Ask for open source projects, or leave them out.
    License(Stance),
    /// Switch an advanced exclusion on or off.
    Advanced(String),
    HideInstalled(bool),
    /// Release a filter the game provides, so the person can choose another.
    Unlock(Lock),
    /// Take the game's filter back and forget the person's own for it.
    Sync(Lock),
    /// Remember whether the advanced exclusions are open (no new search).
    AdvancedOpen(bool),
    /// Drop every filter the person picked.
    ClearFilters,
}

/// What a game provides to the search: its version, and its loader where the
/// kind of content has one that matters.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Provided {
    pub version: Option<String>,
    pub loader: Option<String>,
}

impl Provided {
    /// Modrinth App's `instanceFilters`: resource packs are not tied to a
    /// loader or version; a vanilla game has no shaders by version but wants
    /// the vanilla shader loader; mods follow the game's loader when it is one
    /// of the four mod loaders.
    pub fn for_game(kind: ProjectKind, game_version: &str, loader: Loader) -> Self {
        let vanilla_shader = kind == ProjectKind::Shader && loader == Loader::Vanilla;
        let version = (kind != ProjectKind::ResourcePack && !vanilla_shader)
            .then(|| game_version.to_owned())
            .filter(|version| !version.is_empty());
        let loader = match (kind, loader) {
            (_, Loader::Vanilla) if vanilla_shader => Some("vanilla"),
            (ProjectKind::Mod, Loader::Fabric) => Some("fabric"),
            (ProjectKind::Mod, Loader::Forge) => Some("forge"),
            (ProjectKind::Mod, Loader::Quilt) => Some("quilt"),
            (ProjectKind::Mod, Loader::NeoForge) => Some("neoforge"),
            _ => None,
        };
        Self {
            version,
            loader: loader.map(str::to_owned),
        }
    }
}

/// The kinds the tabs offer. Inside a game, packs are not installed into it,
/// and a vanilla game takes no mods.
pub fn visible_kinds(game: Option<Loader>) -> Vec<ProjectKind> {
    let all = [
        ProjectKind::Modpack,
        ProjectKind::Mod,
        ProjectKind::ResourcePack,
        ProjectKind::Shader,
    ];
    match game {
        None => all.to_vec(),
        Some(loader) => all
            .into_iter()
            .filter(|kind| match kind {
                ProjectKind::Modpack => false,
                ProjectKind::Mod => loader != Loader::Vanilla,
                _ => true,
            })
            .collect(),
    }
}

/// What the Discover page is asking for. The page state *is* this value, so a
/// refresh of the list can never disagree with the controls.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoverQuery {
    pub kind: ProjectKind,
    pub text: String,
    pub sort: SortIndex,
    /// Zero-based.
    pub page: u32,
    pub page_size: u32,
    pub versions: Vec<String>,
    pub loaders: Vec<Pick>,
    pub categories: Vec<Pick>,
    pub client: bool,
    pub server: bool,
    pub license: Option<Stance>,
    /// Advanced exclusions switched on, by option id.
    pub advanced: Vec<String>,
    /// The version list also shows snapshots.
    pub show_all_versions: bool,
    pub hide_installed: bool,
    /// Filters the game provides that the person released.
    pub unlocked: Vec<Lock>,
}

fn toggle<T: PartialEq>(list: &mut Vec<T>, item: T) {
    match list.iter().position(|have| *have == item) {
        Some(at) => {
            list.remove(at);
        }
        None => list.push(item),
    }
}

impl DiscoverQuery {
    pub fn new(kind: ProjectKind) -> Self {
        Self {
            kind,
            text: String::new(),
            sort: SortIndex::Relevance,
            page: 0,
            page_size: 20,
            versions: Vec::new(),
            loaders: Vec::new(),
            categories: Vec::new(),
            client: false,
            server: false,
            license: None,
            advanced: Vec::new(),
            show_all_versions: false,
            hide_installed: false,
            unlocked: Vec::new(),
        }
    }

    /// A new page for this kind that keeps what survives a change of kind:
    /// the version choice, the license, and the advanced exclusions.
    fn for_kind(&self, kind: ProjectKind) -> Self {
        let mut next = Self::new(kind);
        next.page_size = self.page_size;
        next.versions = self.versions.clone();
        next.license = self.license;
        next.advanced = self.advanced.clone();
        next.show_all_versions = self.show_all_versions;
        next.hide_installed = self.hide_installed;
        next.unlocked = self.unlocked.clone();
        next.advanced
            .retain(|id| advanced_options(kind).iter().any(|option| option.id == *id));
        next
    }

    /// Applies one change. Anything but paging returns to the first page, and
    /// a different project type starts its own search: relevance, no text, no
    /// loaders or categories (they do not carry over).
    #[must_use]
    pub fn apply(mut self, change: DiscoverChange) -> Self {
        if let DiscoverChange::Page(page) = change {
            self.page = page;
            return self;
        }
        self.page = 0;
        match change {
            DiscoverChange::Kind(kind) => {
                if kind != self.kind {
                    self = self.for_kind(kind);
                }
            }
            DiscoverChange::Sort(sort) => self.sort = sort,
            DiscoverChange::PageSize(size) => self.page_size = size.clamp(1, 100),
            DiscoverChange::ToggleVersion(version) => toggle(&mut self.versions, version),
            DiscoverChange::ShowAllVersions(all) => self.show_all_versions = all,
            DiscoverChange::Include(group, name) => {
                let list = self.picks_mut(group);
                match list.iter().position(|pick| pick.name == name) {
                    Some(at) => {
                        list.remove(at);
                    }
                    None => list.push(Pick::include(name)),
                }
            }
            DiscoverChange::Exclude(group, name) => {
                let list = self.picks_mut(group);
                match list.iter().position(|pick| pick.name == name) {
                    Some(at) if list[at].stance == Stance::Exclude => {
                        list.remove(at);
                    }
                    Some(at) => list[at].stance = Stance::Exclude,
                    None => list.push(Pick::exclude(name)),
                }
            }
            DiscoverChange::ToggleSide(Side::Client) => self.client = !self.client,
            DiscoverChange::ToggleSide(Side::Server) => self.server = !self.server,
            DiscoverChange::License(stance) => {
                self.license = (self.license != Some(stance)).then_some(stance);
            }
            DiscoverChange::Advanced(id) => self.toggle_advanced(id),
            DiscoverChange::HideInstalled(hide) => self.hide_installed = hide,
            DiscoverChange::Unlock(lock) => {
                if !self.unlocked.contains(&lock) {
                    self.unlocked.push(lock);
                }
            }
            DiscoverChange::Sync(lock) => {
                self.unlocked.retain(|have| *have != lock);
                match lock {
                    Lock::Version => self.versions.clear(),
                    Lock::Loader => self.loaders.clear(),
                }
            }
            DiscoverChange::ClearFilters => {
                self.versions.clear();
                self.loaders.clear();
                self.categories.clear();
                self.client = false;
                self.server = false;
                self.license = None;
                self.advanced.clear();
            }
            DiscoverChange::AdvancedOpen(_) | DiscoverChange::Page(_) => {}
        }
        self
    }

    fn picks_mut(&mut self, group: PickGroup) -> &mut Vec<Pick> {
        match group {
            PickGroup::Loader => &mut self.loaders,
            PickGroup::Category => &mut self.categories,
        }
    }

    /// Choosing an option clears its parent and its children, as in the
    /// app's filter list (`setFilter`).
    fn toggle_advanced(&mut self, id: String) {
        if let Some(at) = self.advanced.iter().position(|have| *have == id) {
            self.advanced.remove(at);
            return;
        }
        for option in advanced_options(self.kind) {
            if option.id == id {
                self.advanced.retain(|have| {
                    Some(have.as_str()) != option.parent
                        && advanced_options(self.kind)
                            .iter()
                            .find(|other| other.id == *have)
                            .is_none_or(|other| other.parent != Some(id.as_str()))
                });
            }
        }
        self.advanced.push(id);
    }

    pub fn is_unlocked(&self, lock: Lock) -> bool {
        self.unlocked.contains(&lock)
    }

    /// Whether the version list is currently the game's (locked).
    pub fn version_locked(&self, provided: &Provided) -> bool {
        provided.version.is_some() && !self.is_unlocked(Lock::Version)
    }

    pub fn loader_locked(&self, provided: &Provided) -> bool {
        provided.loader.is_some() && !self.is_unlocked(Lock::Loader)
    }

    /// Whether the person has narrowed the search themselves.
    pub fn filtered(&self) -> bool {
        !(self.versions.is_empty()
            && self.loaders.is_empty()
            && self.categories.is_empty()
            && !self.client
            && !self.server
            && self.license.is_none()
            && self.advanced.is_empty())
    }

    /// The request for the core. `hidden` is the projects to leave out when
    /// "hide already installed" is on (the application looks them up).
    pub fn to_search(
        &self,
        provided: &Provided,
        filters: &FilterModel,
        hidden: Vec<String>,
    ) -> SearchQuery {
        let mut search = SearchQuery::new(self.kind);
        search.text = self.text.clone();
        search.sort = self.sort;
        search.page = self.page;
        search.page_size = self.page_size.clamp(1, 100);
        search.game_versions = match (&provided.version, self.version_locked(provided)) {
            (Some(version), true) => vec![version.clone()],
            _ => self.versions.clone(),
        };
        search.loaders = match (&provided.loader, self.loader_locked(provided)) {
            (Some(loader), true) => vec![Pick::include(loader.clone()).any_of()],
            _ => self
                .loaders
                .iter()
                .cloned()
                .map(|pick| {
                    if pick.stance == Stance::Include {
                        pick.any_of()
                    } else {
                        pick
                    }
                })
                .collect(),
        };
        search.categories = self
            .categories
            .iter()
            .cloned()
            .map(|pick| {
                if filters.any_of(self.kind, &pick.name) {
                    pick.any_of()
                } else {
                    pick
                }
            })
            .collect();
        search.client = self.client;
        search.server = self.server;
        search.open_source = self.license;
        search.hidden_projects = hidden;
        for id in &self.advanced {
            if !advanced_options(self.kind)
                .iter()
                .any(|option| option.id == *id)
            {
                continue;
            }
            if is_type_exclusion(id) {
                search.excluded_types.push(id.clone());
            } else {
                search.excluded_disclosures.push(id.clone());
            }
        }
        search
    }
}
