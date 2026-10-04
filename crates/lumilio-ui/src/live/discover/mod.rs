mod card;
mod filters;
mod query;

#[cfg(test)]
mod tests;

pub use self::card::{CardTag, CardTags, card_tags};
pub use self::filters::{
    AdvancedOption, EPILEPSY_TRIGGERS, FilterModel, Section, advanced_options, default_loaders,
    is_type_exclusion, section_title, sections,
};
pub use self::query::{
    DiscoverChange, DiscoverQuery, Lock, PickGroup, Provided, Side, visible_kinds,
};

use super::library::{relative_time, seed_of};
use lumilio_core::{Environment, ProjectKind, SearchHit, SearchPage, SortIndex};

/// Which date a card shows: when the project came out, or when it last
/// changed (the first when the list is sorted by newest).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DateKind {
    Published,
    Updated,
}

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
    pub tags: CardTags,
    pub downloads: String,
    pub follows: String,
    pub date: String,
    pub date_kind: DateKind,
    /// The project's public page, for the context menu.
    pub page_url: String,
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

pub fn search_row(
    hit: &SearchHit,
    now: u64,
    query: &DiscoverQuery,
    filters: &FilterModel,
) -> SearchRow {
    let date_kind = if query.sort == SortIndex::Newest {
        DateKind::Published
    } else {
        DateKind::Updated
    };
    let date = match date_kind {
        DateKind::Published => &hit.published,
        DateKind::Updated => &hit.updated,
    };
    SearchRow {
        project_id: hit.project_id.clone(),
        kind: hit.kind,
        slug: hit.slug.clone(),
        title: hit.title.clone(),
        author: hit.author.clone(),
        summary: hit.description.clone(),
        environment: hit.environment,
        tags: card_tags(hit, query, filters),
        downloads: count_label(hit.downloads),
        follows: count_label(hit.follows),
        date: parse_rfc3339(date).map_or_else(String::new, |at| relative_time(at, now)),
        date_kind,
        page_url: hit.page_url(),
        icon_url: hit.icon_url.clone(),
        seed: seed_of(&hit.project_id),
    }
}

pub fn search_rows(
    page: &SearchPage,
    now: u64,
    query: &DiscoverQuery,
    filters: &FilterModel,
) -> Vec<SearchRow> {
    page.hits
        .iter()
        .map(|hit| search_row(hit, now, query, filters))
        .collect()
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

pub const PAGE_SIZES: [u32; 6] = [5, 10, 15, 20, 50, 100];

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
        "vanilla" => "原版",
        "iris" => "Iris",
        "optifine" => "OptiFine",
        "liteloader" => "LiteLoader",
        "babric" => "Babric",
        "plugin" => "插件",
        "datapack" => "数据包",
        "features" => "特性",
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
        Environment::ClientOrServer => "客户端或服务端",
        Environment::ClientAndServer => "客户端和服务端",
        Environment::ClientOnly => "客户端",
        Environment::ServerOnly => "服务端",
        Environment::SingleplayerOnly => "单人游戏",
        Environment::DedicatedServerOnly => "专用服务器",
    }
}
