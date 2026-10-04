//! The choices the Discover sidebar offers, per kind of content.
//!
//! Modeled on Modrinth App's filter list (`useSearch` in
//! `packages/ui/src/utils/search.ts`, GPL-3.0-only; ADR 0022): which sections
//! each kind has and in what order, which loaders come first, how categories
//! are grouped and ordered, and the advanced exclusions.

use lumilio_core::{DiscoverFilters, GameVersionTag, LoaderTag, ProjectKind};

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
        option("ai_content", "AI 生成内容"),
        under("ai_content", "ai_content_code", "AI 代码"),
        under("ai_content", "ai_content_assets", "AI 素材"),
        under("ai_content", "ai_content_text", "AI 文本"),
        option("ai_functionality", "生成式 AI 功能"),
        option("advertisements", "广告"),
        option(EPILEPSY_TRIGGERS, "光敏性触发"),
    ];
    if runs_code {
        options.push(option("system_interactions", "与外部系统交互"));
        options.push(option("telemetry", "遥测"));
        options.push(under("telemetry", "telemetry_opt_in", "遥测·选择加入"));
        options.push(under("telemetry", "telemetry_opt_out", "遥测·可以退出"));
        options.push(under(
            "telemetry",
            "telemetry_always_active",
            "遥测·始终启用",
        ));
    }
    options.push(option("paid_features", "付费功能"));
    options.push(option("archived", "已归档"));
    if kind == ProjectKind::Mod {
        options.push(option("plugin", "插件"));
        options.push(option("datapack", "数据包"));
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FilterModel {
    pub loaded: bool,
    pub error: Option<String>,
    /// Game versions newest first, with whether each is a full release.
    versions: Vec<(String, bool)>,
    tags: Vec<GameVersionTag>,
    categories: Vec<(ProjectKind, String, String)>,
    loaders: Vec<LoaderTag>,
}

impl FilterModel {
    pub fn from_core(filters: &DiscoverFilters) -> Self {
        Self {
            loaded: true,
            error: None,
            versions: filters
                .game_versions
                .iter()
                .map(|tag| (tag.version.clone(), tag.release))
                .collect(),
            tags: filters.game_versions.clone(),
            categories: filters
                .categories
                .iter()
                .map(|tag| (tag.kind, tag.header.clone(), tag.name.clone()))
                .collect(),
            loaders: filters.loaders.clone(),
        }
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

    /// The category groups of a kind: groups by header (alphabetical), each
    /// ordered the way Modrinth's `sortedCategories` does.
    pub fn category_groups(&self, kind: ProjectKind) -> Vec<(String, Vec<&str>)> {
        let mut headers: Vec<&str> = self
            .categories
            .iter()
            .filter(|(tag_kind, _, _)| *tag_kind == kind)
            .map(|(_, header, _)| header.as_str())
            .collect();
        headers.sort_unstable();
        headers.dedup();
        headers
            .into_iter()
            .map(|header| {
                let mut names: Vec<&str> = self
                    .categories
                    .iter()
                    .filter(|(tag_kind, tag_header, _)| *tag_kind == kind && tag_header == header)
                    .map(|(_, _, name)| name.as_str())
                    .collect();
                names.sort_by(|a, b| category_order(header, a, b));
                (header.to_owned(), names)
            })
            .collect()
    }

    /// Whether picking this category means "any of the group" (resolutions).
    pub fn any_of(&self, kind: ProjectKind, name: &str) -> bool {
        self.categories.iter().any(|(tag_kind, header, tag)| {
            *tag_kind == kind && tag == name && header == "resolutions"
        })
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
            return list;
        }
        ProjectKind::ResourcePack => {
            list.extend(categories);
            list.push(Section::Version);
        }
    }
    list.push(Section::License);
    list.push(Section::Advanced);
    list
}

/// Section titles.
pub fn section_title(section: &Section) -> String {
    match section {
        Section::Version => "游戏版本".to_owned(),
        Section::Loader => "加载器".to_owned(),
        Section::Environment => "运行环境".to_owned(),
        Section::License => "许可证".to_owned(),
        Section::Advanced => "高级排除".to_owned(),
        Section::Category(header) => match header.as_str() {
            "categories" => "分类".to_owned(),
            "features" => "特性".to_owned(),
            "resolutions" => "分辨率".to_owned(),
            "performance impact" => "性能影响".to_owned(),
            other => super::tag_label(other),
        },
    }
}
