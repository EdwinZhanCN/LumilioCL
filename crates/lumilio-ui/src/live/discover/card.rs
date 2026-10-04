//! The tags on a result card, chosen the way Modrinth App's `ProjectCardTags`
//! does (GPL-3.0-only; ADR 0022): categories first in name order, then
//! loaders with the usual ones first; what the person already filtered by is
//! not repeated; the rest folds into an overflow count.

use super::filters::FilterModel;
use super::query::DiscoverQuery;
use lumilio_core::{ProjectKind, SearchHit};

/// Loaders a card lists before the others.
const USUAL_LOADERS: [&str; 6] = ["fabric", "forge", "neoforge", "iris", "optifine", "vanilla"];

/// How many tags a list card shows next to its install button; one more when
/// it has no environment tag.
const SHOWN_WITH_ACTION: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CardTag {
    pub name: String,
    pub loader: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CardTags {
    pub shown: Vec<CardTag>,
    /// Tags that did not fit or were left out, as the `+N` count.
    pub overflow: Vec<String>,
}

pub fn card_tags(hit: &SearchHit, query: &DiscoverQuery, filters: &FilterModel) -> CardTags {
    let mut categories: Vec<&str> = hit.categories.iter().map(String::as_str).collect();
    categories.sort_unstable();
    categories.dedup();
    let mut loaders: Vec<&str> = hit.loaders.iter().map(String::as_str).collect();
    loaders.sort_by(|a, b| {
        let usual = |name: &str| !USUAL_LOADERS.contains(&name);
        usual(a).cmp(&usual(b)).then_with(|| a.cmp(b))
    });
    loaders.dedup();
    let tags: Vec<CardTag> = categories
        .into_iter()
        .map(|name| CardTag {
            name: name.to_owned(),
            loader: false,
        })
        .chain(loaders.into_iter().map(|name| CardTag {
            name: name.to_owned(),
            loader: true,
        }))
        .collect();

    // What the person filtered by is already known to them, as are loaders
    // that do not serve this kind; a loader filter hides the loaders.
    let filtered: Vec<&str> = query
        .categories
        .iter()
        .chain(&query.loaders)
        .map(|pick| pick.name.as_str())
        .collect();
    let not_for_kind = filters.loaders_not_for(hit.kind);
    let hide_loaders = !query.loaders.is_empty() || matches!(hit.kind, ProjectKind::ResourcePack);
    let limit = SHOWN_WITH_ACTION + usize::from(hit.environment.is_none());
    let shown: Vec<CardTag> = tags
        .iter()
        .filter(|tag| {
            !filtered.contains(&tag.name.as_str())
                && !not_for_kind.contains(&tag.name.as_str())
                && !(hide_loaders && tag.loader)
        })
        .take(limit)
        .cloned()
        .collect();
    let overflow = tags
        .iter()
        .filter(|tag| !shown.contains(tag))
        .map(|tag| tag.name.clone())
        .collect();
    CardTags { shown, overflow }
}
