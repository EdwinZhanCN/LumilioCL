//! The choices the Discover sidebar offers, per kind of content.
//!
//! Modeled on Modrinth App's filter list (`useSearch` in
//! `packages/ui/src/utils/search.ts`, GPL-3.0-only; ADR 0022): which sections
//! each kind has and in what order, which loaders come first, how categories
//! are grouped and ordered, and the advanced exclusions.

use lumilio_core::{
    DiscoverFilters, GameVersionTag, KindAbilities, LoaderTag, ProjectKind, SortIndex,
};

/// The photosensitivity exclusion, which asks for a warning the first time.
pub const EPILEPSY_TRIGGERS: &str = "epilepsy_triggers";

/// A sidebar section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Section {
    Version,
    Loader,
    /// One group of categories, by Modrinth's header (`categories`, …).
    Category(String),
    Environment,
    License,
    Advanced,
}

/// One advanced exclusion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdvancedOption {
    pub id: &'static str,
    pub label: &'static str,
    /// A finer choice under another one.
    pub parent: Option<&'static str>,
}

const fn option(id: &'static str, label: &'static str) -> AdvancedOption {
    AdvancedOption {
        id,
        label,
        parent: None,
    }
}

const fn under(parent: &'static str, id: &'static str, label: &'static str) -> AdvancedOption {
    AdvancedOption {
        id,
        label,
        parent: Some(parent),
    }
}

/// Disclosures first (those that make sense for the kind), then the other
/// project types a mod may also be.
pub fn advanced_options(kind: ProjectKind) -> Vec<AdvancedOption> {
    let runs_code = matches!(kind, ProjectKind::Mod | ProjectKind::Modpack);
    let mut options = vec![
        option("ai_content", crate::tr!("discover-exclude-ai-content")),
        under(
            "ai_content",
            "ai_content_code",
            crate::tr!("discover-exclude-ai-content-code"),
        ),
        under(
            "ai_content",
            "ai_content_assets",
            crate::tr!("discover-exclude-ai-content-assets"),
        ),
        under(
            "ai_content",
            "ai_content_text",
            crate::tr!("discover-exclude-ai-content-text"),
        ),
        option(
            "ai_functionality",
            crate::tr!("discover-exclude-ai-functionality"),
        ),
        option(
            "advertisements",
            crate::tr!("discover-exclude-advertisements"),
        ),
        option(EPILEPSY_TRIGGERS, crate::tr!("discover-exclude-epilepsy")),
    ];
    if runs_code {
        options.push(option(
            "system_interactions",
            crate::tr!("discover-exclude-system-interactions"),
        ));
        options.push(option(
            "telemetry",
            crate::tr!("discover-exclude-telemetry"),
        ));
        options.push(under(
            "telemetry",
            "telemetry_opt_in",
            crate::tr!("discover-exclude-telemetry-opt-in"),
        ));
        options.push(under(
            "telemetry",
            "telemetry_opt_out",
            crate::tr!("discover-exclude-telemetry-opt-out"),
        ));
        options.push(under(
            "telemetry",
            "telemetry_always_active",
            crate::tr!("discover-exclude-telemetry-always-active"),
        ));
    }
    options.push(option(
        "paid_features",
        crate::tr!("discover-exclude-paid-features"),
    ));
    options.push(option("archived", crate::tr!("discover-exclude-archived")));
    if kind == ProjectKind::Mod {
        options.push(option("plugin", crate::tr!("discover-exclude-plugin")));
        options.push(option("datapack", crate::tr!("discover-exclude-datapack")));
    }
    options
}

/// Whether the option leaves out another project *type*, not a disclosure.
pub fn is_type_exclusion(id: &str) -> bool {
    matches!(id, "plugin" | "datapack")
}

/// Loaders the loader list shows first; the rest hide behind "show more".
pub fn default_loaders(kind: ProjectKind) -> &'static [&'static str] {
    match kind {
        ProjectKind::Mod => &["fabric", "forge", "neoforge"],
        ProjectKind::Shader => &["iris", "optifine", "vanilla"],
        _ => &[],
    }
}

/// The loaders for a kind when Modrinth's list has not arrived.
fn fallback_loaders(kind: ProjectKind) -> &'static [&'static str] {
    match kind {
        ProjectKind::Mod | ProjectKind::Modpack => &["fabric", "forge", "neoforge", "quilt"],
        ProjectKind::Shader => &["iris", "optifine", "vanilla", "canvas"],
        ProjectKind::ResourcePack => &[],
    }
}

/// A category the sidebar offers: what a search asks for (`id`) and what
/// the person reads (`name`, which may still be a catalog key).
#[derive(Clone, Debug, Eq, PartialEq)]
struct CategoryOption {
    kind: ProjectKind,
    header: String,
    id: String,
    name: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FilterModel {
    pub loaded: bool,
    pub error: Option<String>,
    /// Game versions newest first, with whether each is a full release.
    versions: Vec<(String, bool)>,
    tags: Vec<GameVersionTag>,
    categories: Vec<CategoryOption>,
    loaders: Vec<LoaderTag>,
    /// What the content source can filter by; empty until it has said, which
    /// offers everything.
    abilities: Vec<KindAbilities>,
}

impl FilterModel {
    pub fn from_core(filters: &DiscoverFilters) -> Self {
        Self {
            loaded: true,
            error: None,
            abilities: filters.abilities.clone(),
            versions: filters
                .game_versions
                .iter()
                .map(|tag| (tag.version.clone(), tag.release))
                .collect(),
            tags: filters.game_versions.clone(),
            categories: filters
                .categories
                .iter()
                .map(|tag| CategoryOption {
                    kind: tag.kind,
                    header: tag.header.clone(),
                    id: tag.id.clone(),
                    name: tag.name.clone(),
                })
                .collect(),
            loaders: filters.loaders.clone(),
        }
    }

    /// Whether the source can filter this kind by `section`; a source that has
    /// not said yet offers everything.
    pub fn offers(&self, kind: ProjectKind, section: &Section) -> bool {
        if self.abilities.is_empty() {
            return true;
        }
        let Some(ability) = self.abilities.iter().find(|ability| ability.kind == kind) else {
            return false;
        };
        match section {
            Section::Version => ability.game_versions,
            Section::Loader => ability.loaders,
            Section::Environment => ability.environment,
            Section::License => ability.open_source,
            Section::Advanced => ability.advanced,
            Section::Category(_) => ability.categories,
        }
    }

    /// The sorts the source offers for a kind, in menu order; every sort
    /// until it has said.
    pub fn sorts(&self, kind: ProjectKind) -> Vec<SortIndex> {
        let offered = self
            .abilities
            .iter()
            .find(|ability| ability.kind == kind)
            .map(|ability| ability.sorts.clone());
        super::SORTS
            .iter()
            .copied()
            .filter(|sort| {
                offered
                    .as_ref()
                    .is_none_or(|offered| offered.is_empty() || offered.contains(sort))
            })
            .collect()
    }

    /// Whether a picked category or loader can be turned into "leave out".
    pub fn offers_exclusions(&self, kind: ProjectKind) -> bool {
        self.abilities.is_empty()
            || self
                .abilities
                .iter()
                .any(|ability| ability.kind == kind && ability.exclusions)
    }

    /// The source's own page listing this kind, if it said.
    pub fn browse_url(&self, kind: ProjectKind) -> Option<&str> {
        self.abilities
            .iter()
            .find(|ability| ability.kind == kind)
            .and_then(|ability| ability.browse_url.as_deref())
    }

    /// The name of a category by the ID a search asks for.
    pub fn category_name(&self, kind: ProjectKind, id: &str) -> Option<&str> {
        self.categories
            .iter()
            .find(|option| option.kind == kind && option.id == id)
            .map(|option| option.name.as_str())
    }

    /// Whether the source can leave out what the game already has.
    pub fn offers_hide_installed(&self, kind: ProjectKind) -> bool {
        self.abilities.is_empty()
            || self
                .abilities
                .iter()
                .any(|ability| ability.kind == kind && ability.hide_installed)
    }

    /// Modrinth's whole game version list, newest first.
    pub fn game_tags(&self) -> &[GameVersionTag] {
        &self.tags
    }

    /// Releases, or every version (snapshots too) when `all`.
    pub fn versions(&self, all: bool) -> Vec<&str> {
        self.versions
            .iter()
            .filter(|(_, release)| all || *release)
            .map(|(version, _)| version.as_str())
            .collect()
    }

    /// Whether Modrinth knows this version (a chosen snapshot stays listed
    /// when "all versions" is off).
    pub fn knows_version(&self, version: &str) -> bool {
        self.versions.iter().any(|(name, _)| name == version)
    }

    /// The category groups of a kind, as (ID, name) pairs: groups by header
    /// (alphabetical), each ordered by name the way Modrinth's
    /// `sortedCategories` does.
    pub fn category_groups(&self, kind: ProjectKind) -> Vec<(String, Vec<(&str, &str)>)> {
        let mut headers: Vec<&str> = self
            .categories
            .iter()
            .filter(|option| option.kind == kind)
            .map(|option| option.header.as_str())
            .collect();
        headers.sort_unstable();
        headers.dedup();
        headers
            .into_iter()
            .map(|header| {
                let mut options: Vec<(&str, &str)> = self
                    .categories
                    .iter()
                    .filter(|option| option.kind == kind && option.header == header)
                    .map(|option| (option.id.as_str(), option.name.as_str()))
                    .collect();
                options.sort_by(|a, b| category_order(header, a.1, b.1));
                (header.to_owned(), options)
            })
            .collect()
    }

    /// Whether picking this category (by ID) means "any of the group"
    /// (resolutions).
    pub fn any_of(&self, kind: ProjectKind, id: &str) -> bool {
        self.categories
            .iter()
            .any(|option| option.kind == kind && option.id == id && option.header == "resolutions")
    }

    /// The loaders to list for a kind, the usual ones first.
    pub fn loader_options(&self, kind: ProjectKind) -> Vec<String> {
        let serves = |tag: &LoaderTag| {
            let has = |name: &str| tag.project_types.iter().any(|ty| ty == name);
            match kind {
                ProjectKind::Mod => has("mod") && !has("plugin") && !has("datapack"),
                ProjectKind::Modpack => has("modpack"),
                ProjectKind::Shader => has("shader"),
                ProjectKind::ResourcePack => false,
            }
        };
        let mut names: Vec<String> = self
            .loaders
            .iter()
            .filter(|tag| serves(tag))
            .map(|tag| tag.name.clone())
            .collect();
        if names.is_empty() {
            names = fallback_loaders(kind)
                .iter()
                .map(|name| (*name).to_owned())
                .collect();
        }
        let defaults = default_loaders(kind);
        // Stable: the usual ones first, the rest as Modrinth lists them.
        names.sort_by_key(|name| !defaults.contains(&name.as_str()));
        names
    }

    /// Loaders that do not serve this kind of project; cards leave them out of
    /// their tags.
    pub fn loaders_not_for(&self, kind: ProjectKind) -> Vec<&str> {
        self.loaders
            .iter()
            .filter(|tag| {
                !tag.project_types
                    .iter()
                    .any(|ty| ty == kind.protocol_name())
            })
            .map(|tag| tag.name.as_str())
            .collect()
    }
}

/// Modrinth's category order inside a group: performance impact by weight,
/// the rest by name with numbers in numeric order.
fn category_order(header: &str, a: &str, b: &str) -> std::cmp::Ordering {
    if header == "performance impact" {
        const WEIGHT: [&str; 5] = ["potato", "low", "medium", "high", "screenshot"];
        let at = |name: &str| WEIGHT.iter().position(|weight| *weight == name);
        return at(a).cmp(&at(b));
    }
    if a == "pokemon" {
        return std::cmp::Ordering::Less;
    }
    if b == "pokemon" {
        return std::cmp::Ordering::Greater;
    }
    natural(a, b)
}

/// Compares names, reading runs of digits as numbers (`16x` before `128x`).
fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    let number = |text: &str| -> (u64, usize) {
        let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
        (digits.parse().unwrap_or(0), digits.len())
    };
    let (mut a, mut b) = (a, b);
    loop {
        match (a.chars().next(), b.chars().next()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let ((left, left_len), (right, right_len)) = (number(a), number(b));
                if left != right {
                    return left.cmp(&right);
                }
                a = &a[left_len..];
                b = &b[right_len..];
            }
            (Some(x), Some(y)) => {
                let order = x.to_lowercase().cmp(y.to_lowercase());
                if order != std::cmp::Ordering::Equal {
                    return order;
                }
                a = &a[x.len_utf8()..];
                b = &b[y.len_utf8()..];
            }
        }
    }
}

/// The sections of a kind, in the order the sidebar lists them. This is the
/// app's own ordering: its `ordering` weights put the game version and the
/// loader first for mods, and the version after the loader for shaders.
pub fn sections(kind: ProjectKind, filters: &FilterModel) -> Vec<Section> {
    let categories = filters
        .category_groups(kind)
        .into_iter()
        .map(|(header, _)| Section::Category(header));
    let mut list = Vec::new();
    match kind {
        ProjectKind::Mod => {
            list.push(Section::Version);
            list.push(Section::Loader);
            list.extend(categories);
            list.push(Section::Environment);
        }
        ProjectKind::Modpack => {
            list.extend(categories);
            list.push(Section::Environment);
            list.push(Section::Version);
            list.push(Section::Loader);
        }
        ProjectKind::Shader => {
            list.extend(categories);
            list.push(Section::Loader);
            list.push(Section::License);
            list.push(Section::Version);
            list.push(Section::Advanced);
            list.retain(|section| filters.offers(kind, section));
            return list;
        }
        ProjectKind::ResourcePack => {
            list.extend(categories);
            list.push(Section::Version);
        }
    }
    list.push(Section::License);
    list.push(Section::Advanced);
    list.retain(|section| filters.offers(kind, section));
    list
}

/// Section titles.
pub fn section_title(section: &Section) -> String {
    match section {
        Section::Version => crate::tr!("discover-section-version").to_owned(),
        Section::Loader => crate::tr!("discover-section-loader").to_owned(),
        Section::Environment => crate::tr!("discover-section-environment").to_owned(),
        Section::License => crate::tr!("discover-section-license").to_owned(),
        Section::Advanced => crate::tr!("discover-section-advanced").to_owned(),
        Section::Category(header) => match header.as_str() {
            "categories" => crate::tr!("discover-section-categories").to_owned(),
            "features" => crate::tr!("discover-section-features").to_owned(),
            "resolutions" => crate::tr!("discover-section-resolutions").to_owned(),
            "performance impact" => crate::tr!("discover-section-performance").to_owned(),
            other => super::tag_label(other),
        },
    }
}
