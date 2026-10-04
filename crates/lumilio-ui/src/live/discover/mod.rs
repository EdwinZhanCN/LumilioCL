use super::library::{relative_time, seed_of};
use lumilio_core::{
    DiscoverFilters, Environment, ProjectKind, SearchHit, SearchPage, SearchQuery, SortIndex,
    environment,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchRow {
    /// Modrinth's id, to recognise what the game already has.
    pub project_id: String,
    pub kind: ProjectKind,
    pub slug: String,
    pub title: String,
    pub author: String,
    pub summary: String,
    pub environment: Option<Environment>,
    /// Category names as Modrinth spells them.
    pub categories: Vec<String>,
    pub loaders: Vec<String>,
    pub downloads: String,
    pub follows: String,
    pub updated: String,
    pub icon_url: Option<String>,
    /// Seeds the placeholder cover while (or instead of) the real icon.
    pub seed: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum SearchStatus {
    /// Nothing searched yet.
    #[default]
    Idle,
    Searching,
    Failed(String),
    Done {
        total: u64,
    },
}

pub fn count_label(count: u64) -> String {
    match count {
        0..10_000 => count.to_string(),
        10_000..100_000_000 => format!("{:.1} 万", count as f64 / 10_000.0),
        _ => format!("{:.1} 亿", count as f64 / 100_000_000.0),
    }
}

pub fn search_row(hit: &SearchHit, now: u64) -> SearchRow {
    SearchRow {
        project_id: hit.project_id.clone(),
        kind: hit.kind,
        slug: hit.slug.clone(),
        title: hit.title.clone(),
        author: hit.author.clone(),
        summary: hit.description.clone(),
        environment: environment(hit.client_side, hit.server_side),
        categories: hit.categories.clone(),
        loaders: hit.loaders.clone(),
        downloads: count_label(hit.downloads),
        follows: count_label(hit.follows),
        updated: parse_rfc3339(&hit.updated).map_or_else(String::new, |at| relative_time(at, now)),
        icon_url: hit.icon_url.clone(),
        seed: seed_of(&hit.project_id),
    }
}

pub fn search_rows(page: &SearchPage, now: u64) -> Vec<SearchRow> {
    page.hits.iter().map(|hit| search_row(hit, now)).collect()
}

/// Seconds since the epoch for `2026-09-27T10:00:00Z` and the offset/fraction
/// variants Modrinth sends; `None` for anything else.
pub fn parse_rfc3339(text: &str) -> Option<u64> {
    let (date, time) = text.split_once('T')?;
    let mut ymd = date.split('-').map(|part| part.parse::<i64>().ok());
    let (year, month, day) = (ymd.next()??, ymd.next()??, ymd.next()??);
    if ymd.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let clock: String = time
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == ':')
        .collect();
    let mut hms = clock.split(':').map(|part| part.parse::<i64>().ok());
    let (hour, minute, second) = (
        hms.next()??,
        hms.next()??,
        hms.next().flatten().unwrap_or(0),
    );
    // Days since 1970-01-01 (proleptic Gregorian, Hinnant's algorithm).
    let shifted = if month <= 2 { year - 1 } else { year };
    let era = shifted.div_euclid(400);
    let year_of_era = shifted - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    u64::try_from(days * 86_400 + hour * 3600 + minute * 60 + second).ok()
}

pub const PAGE_SIZES: [u32; 4] = [10, 20, 50, 100];

pub const SORTS: [SortIndex; 5] = [
    SortIndex::Relevance,
    SortIndex::Downloads,
    SortIndex::Follows,
    SortIndex::Newest,
    SortIndex::Updated,
];

pub const fn sort_label(sort: SortIndex) -> &'static str {
    match sort {
        SortIndex::Relevance => "相关度",
        SortIndex::Downloads => "下载量",
        SortIndex::Follows => "关注数",
        SortIndex::Newest => "最新发布",
        SortIndex::Updated => "最近更新",
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
    pub game_version: Option<String>,
    pub categories: Vec<String>,
    pub loaders: Vec<String>,
}

/// One change to the query made on the page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiscoverChange {
    Kind(ProjectKind),
    Sort(SortIndex),
    PageSize(u32),
    Version(Option<String>),
    ToggleCategory(String),
    ToggleLoader(String),
    Page(u32),
    /// Drop the version, category and loader filters.
    ClearFilters,
}

pub(super) fn toggle(list: &mut Vec<String>, name: String) {
    match list.iter().position(|item| *item == name) {
        Some(at) => {
            list.remove(at);
        }
        None => list.push(name),
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
            game_version: None,
            categories: Vec::new(),
            loaders: Vec::new(),
        }
    }

    /// Applies one change. Anything but paging returns to the first page, and
    /// a different project type forgets its categories and loaders (they do
    /// not carry over).
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
                    self.kind = kind;
                    self.categories.clear();
                    self.loaders.clear();
                }
            }
            DiscoverChange::Sort(sort) => self.sort = sort,
            DiscoverChange::PageSize(size) => self.page_size = size.clamp(1, 100),
            DiscoverChange::Version(version) => self.game_version = version,
            DiscoverChange::ToggleCategory(name) => toggle(&mut self.categories, name),
            DiscoverChange::ToggleLoader(name) => toggle(&mut self.loaders, name),
            DiscoverChange::ClearFilters => {
                self.game_version = None;
                self.categories.clear();
                self.loaders.clear();
            }
            DiscoverChange::Page(_) => {}
        }
        self
    }

    pub fn to_search(&self) -> SearchQuery {
        SearchQuery {
            text: self.text.clone(),
            kind: self.kind,
            game_version: self.game_version.clone(),
            loaders: self.loaders.clone(),
            categories: self.categories.clone(),
            sort: self.sort,
            page: self.page,
            page_size: self.page_size,
        }
    }

    /// Whether any filter narrows the results.
    pub fn filtered(&self) -> bool {
        self.game_version.is_some() || !self.categories.is_empty() || !self.loaders.is_empty()
    }

    /// Only mods and modpacks have loaders worth choosing.
    pub const fn has_loaders(&self) -> bool {
        matches!(self.kind, ProjectKind::Mod | ProjectKind::Modpack)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageItem {
    /// Zero-based page.
    Page(u32),
    Gap,
}

/// The numbers a pager shows: first, last, and the current page with one
/// neighbour each side; a gap of a single page is filled instead of elided.
pub fn page_items(current: u32, pages: u32) -> Vec<PageItem> {
    if pages == 0 {
        return Vec::new();
    }
    let last = pages - 1;
    let current = current.min(last);
    let mut shown: Vec<u32> = vec![0, last, current];
    if current > 0 {
        shown.push(current - 1);
    }
    if current < last {
        shown.push(current + 1);
    }
    shown.sort_unstable();
    shown.dedup();
    let mut items = Vec::new();
    let mut previous: Option<u32> = None;
    for page in shown {
        match previous {
            Some(before) if page == before + 2 => items.push(PageItem::Page(before + 1)),
            Some(before) if page > before + 2 => items.push(PageItem::Gap),
            _ => {}
        }
        items.push(PageItem::Page(page));
        previous = Some(page);
    }
    items
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FilterModel {
    pub loaded: bool,
    pub error: Option<String>,
    /// Release versions, newest first.
    pub versions: Vec<String>,
    categories: Vec<(ProjectKind, String, String)>,
}

impl FilterModel {
    pub fn from_core(filters: &DiscoverFilters) -> Self {
        Self {
            loaded: true,
            error: None,
            versions: filters
                .game_versions
                .iter()
                .filter(|tag| tag.release)
                .map(|tag| tag.version.clone())
                .collect(),
            categories: filters
                .categories
                .iter()
                .map(|tag| (tag.kind, tag.header.clone(), tag.name.clone()))
                .collect(),
        }
    }

    /// Category names for a project type, in Modrinth's order, without the
    /// resolution and performance groups that only some types have (they are
    /// kept: each is a category facet like any other).
    pub fn categories(&self, kind: ProjectKind) -> Vec<&str> {
        self.categories
            .iter()
            .filter(|(tag_kind, _, _)| *tag_kind == kind)
            .map(|(_, _, name)| name.as_str())
            .collect()
    }
}

/// The Chinese name of a Modrinth category, loader or feature tag; unknown
/// ones are shown as Modrinth spells them, capitalized.
pub fn tag_label(name: &str) -> String {
    let known = match name {
        "adventure" => "冒险",
        "cursed" => "诅咒",
        "decoration" => "装饰",
        "economy" => "经济",
        "equipment" => "装备",
        "food" => "食物",
        "game-mechanics" => "游戏机制",
        "library" => "库",
        "magic" => "魔法",
        "management" => "管理",
        "minigame" => "小游戏",
        "mobs" => "生物",
        "optimization" => "优化",
        "social" => "社交",
        "storage" => "存储",
        "technology" => "科技",
        "transportation" => "交通",
        "utility" => "实用",
        "worldgen" => "世界生成",
        "challenging" => "挑战",
        "combat" => "战斗",
        "kitchen-sink" => "大杂烩",
        "lightweight" => "轻量",
        "multiplayer" => "多人",
        "quests" => "任务",
        "audio" => "音频",
        "blocks" => "方块",
        "core-shaders" => "核心着色器",
        "entities" => "实体",
        "environment" => "环境",
        "fonts" => "字体",
        "gui" => "界面",
        "items" => "物品",
        "locale" => "本地化",
        "modded" => "模组",
        "models" => "模型",
        "realistic" => "写实",
        "simplistic" => "极简",
        "themed" => "主题",
        "tweaks" => "调整",
        "vanilla-like" => "原版风格",
        "atmosphere" => "大气",
        "bloom" => "泛光",
        "cartoon" => "卡通",
        "colored-lighting" => "彩色光照",
        "fantasy" => "奇幻",
        "foliage" => "植被",
        "path-tracing" => "路径追踪",
        "pbr" => "PBR",
        "reflections" => "反射",
        "semi-realistic" => "半写实",
        "low" => "低性能影响",
        "medium" => "中性能影响",
        "high" => "高性能影响",
        "potato" => "土豆机",
        "screenshot" => "截图用",
        "fabric" => "Fabric",
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        "quilt" => "Quilt",
        _ => "",
    };
    if !known.is_empty() {
        return known.to_owned();
    }
    let spaced = name.replace('-', " ");
    let mut chars = spaced.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

pub const fn environment_label(environment: Environment) -> &'static str {
    match environment {
        Environment::ClientAndServer => "客户端和服务端",
        Environment::ClientOnly => "客户端",
        Environment::ServerOnly => "服务端",
    }
}

pub const LOADER_CHOICES: [&str; 4] = ["fabric", "forge", "neoforge", "quilt"];
