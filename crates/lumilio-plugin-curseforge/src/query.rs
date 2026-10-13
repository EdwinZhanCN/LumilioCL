//! Search requests and update choices in CurseForge's terms.

use lumilio_plugin_api::content::{Compatibility, LoaderTag, ProjectKind, SearchQuery, Stance};

use crate::protocol::{
    CLASS_MODS, GAME, RawFile, RawMod, before_fabric, channel, class_of, loader_type, sort_field,
};

pub(crate) const MAX_GAME_VERSIONS: u32 = 4;
pub(crate) const MAX_LOADERS: u32 = 5;
pub(crate) const MAX_CATEGORIES: u32 = 10;
/// CurseForge answers no further than this many results into a search.
pub(crate) const RESULT_LIMIT: u64 = 10_000;
/// Most results one search request returns.
pub(crate) const PAGE_LIMIT: u64 = 50;

pub(crate) fn browse_url(kind: ProjectKind) -> String {
    let section = match kind {
        ProjectKind::Mod => "mc-mods",
        ProjectKind::Modpack => "modpacks",
        ProjectKind::ResourcePack => "texture-packs",
        ProjectKind::Shader => "shaders",
    };
    format!("https://www.curseforge.com/minecraft/{section}")
}

/// Where the page starts and how many results it wants, within what
/// CurseForge will answer; `None` past the end.
pub(crate) fn window(query: &SearchQuery) -> Option<(u64, u64)> {
    let size = u64::from(query.page_size.max(1));
    let start = u64::from(query.page).saturating_mul(size);
    (start < RESULT_LIMIT).then(|| (start, size.min(RESULT_LIMIT - start)))
}

fn encode(text: &str) -> String {
    url::form_urlencoded::byte_serialize(text.as_bytes()).collect()
}

/// One name, or a JSON list for several.
fn one_or_many<T: ToString>(single: &str, many: &str, values: &[T]) -> Option<String> {
    match values {
        [] => None,
        [one] => Some(format!("&{single}={}", encode(&one.to_string()))),
        several => {
            let list: Vec<String> = several.iter().map(ToString::to_string).collect();
            let json = format!("[{}]", list.join(","));
            Some(format!("&{many}={}", encode(&json)))
        }
    }
}

/// The search for `size` results from `index`, with the query's filters.
pub(crate) fn search_path(query: &SearchQuery, index: u64, size: u64) -> String {
    let text = query.text.trim();
    let mut path = format!(
        "/v1/mods/search?gameId={GAME}&classId={}&sortField={}&sortOrder=desc&index={index}&pageSize={size}",
        class_of(query.kind),
        sort_field(query.sort, !text.is_empty()),
    );
    if !text.is_empty() {
        path.push_str(&format!("&searchFilter={}", encode(text)));
    }
    let versions: Vec<String> = query
        .game_versions
        .iter()
        .map(|version| format!("\"{}\"", version.replace(['"', '\\'], "")))
        .collect();
    match query.game_versions.as_slice() {
        [] => {}
        [one] => path.push_str(&format!("&gameVersion={}", encode(one))),
        _ => path.push_str(&format!(
            "&gameVersions={}",
            encode(&format!("[{}]", versions.join(",")))
        )),
    }
    let loaders: Vec<u8> = query
        .loaders
        .iter()
        .filter(|pick| pick.stance == Stance::Include)
        .filter_map(|pick| loader_type(&pick.name))
        .collect();
    if let Some(part) = one_or_many("modLoaderType", "modLoaderTypes", &loaders) {
        path.push_str(&part);
    }
    let categories: Vec<u64> = query
        .categories
        .iter()
        .filter(|pick| pick.stance == Stance::Include)
        .filter_map(|pick| pick.name.trim().parse().ok())
        .collect();
    if let Some(part) = one_or_many("categoryId", "categoryIds", &categories) {
        path.push_str(&part);
    }
    path
}

/// The loaders CurseForge can filter mods and modpacks by.
pub(crate) fn loader_tags() -> Vec<LoaderTag> {
    ["fabric", "forge", "neoforge", "quilt"]
        .into_iter()
        .map(|name| LoaderTag {
            name: name.to_owned(),
            kinds: vec![ProjectKind::Mod, ProjectKind::Modpack],
            other_kinds: Vec::new(),
        })
        .collect()
}

/// The newest file of `project` that runs on `compatibility`, is newer than
/// `installed`, and is at least as stable (a release is not offered a beta).
/// The loader counts only for mods; an entry from before 1.14 that names no
/// loader is a Forge mod.
pub(crate) fn newest_compatible(
    project: &RawMod,
    installed: &RawFile,
    compatibility: &Compatibility,
) -> Option<u64> {
    let is_mod = project.class_id == Some(CLASS_MODS);
    let wanted = compatibility
        .loader
        .as_deref()
        .and_then(loader_type)
        .filter(|_| is_mod);
    let floor = channel(installed.release_type);
    project
        .latest_files_indexes
        .iter()
        .filter(|index| index.game_version == compatibility.game_version)
        .filter(|index| match (wanted, index.mod_loader) {
            (None, _) => true,
            (Some(want), Some(have)) => want == have,
            (Some(want), None) => want == 1 && before_fabric(&index.game_version),
        })
        .filter(|index| channel(index.release_type) >= floor)
        .filter(|index| index.file_id > installed.id)
        .map(|index| index.file_id)
        .max()
}
