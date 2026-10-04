use crate::home::WorldHint;
use lumilio_core::{AttentionItem, InstanceRecord, Loader};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibraryCard {
    pub id: String,
    pub name: String,
    /// `1.21.1 · Fabric`.
    pub meta: String,
    pub game_version: String,
    pub favorite: bool,
    /// Never played instances say so instead of showing a time.
    pub played: String,
    /// Stable number the pixel cover is drawn from.
    pub seed: u32,
    pub loader: Loader,
    pub world: WorldHint,
    /// When it was made, for ordering.
    pub created: u64,
    /// The Modrinth modpack it was installed from, if it was.
    pub source_project: Option<String>,
}

/// One of the person's collections: a name and the games filed under it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionRow {
    pub name: String,
    /// Instance ids, in the order they were added.
    pub members: Vec<String>,
}

pub(super) const fn ui_loader(loader: Loader) -> crate::cover::Loader {
    match loader {
        Loader::Vanilla => crate::cover::Loader::Vanilla,
        Loader::Fabric => crate::cover::Loader::Fabric,
        Loader::Forge => crate::cover::Loader::Forge,
        Loader::NeoForge => crate::cover::Loader::NeoForge,
        Loader::Quilt => crate::cover::Loader::Quilt,
    }
}

pub const fn loader_label(loader: Loader) -> &'static str {
    ui_loader(loader).label()
}

/// The sample-side loader, for the cover painter.
pub const fn cover_loader(loader: Loader) -> crate::cover::Loader {
    ui_loader(loader)
}

/// FNV-1a: a stable number from text, so a cover never changes between runs.
pub fn seed_of(text: &str) -> u32 {
    text.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    })
}

pub fn world_of(id: &str) -> WorldHint {
    match seed_of(id) / 7 % 4 {
        0 => WorldHint::Overworld,
        1 => WorldHint::Underground,
        2 => WorldHint::Redstone,
        _ => WorldHint::Nether,
    }
}

/// `刚刚`, `5 分钟前`, `3 小时前`, `昨天`, `4 天前`, `2 个月前`.
pub fn relative_time(then: u64, now: u64) -> String {
    let seconds = now.saturating_sub(then);
    match seconds {
        0..60 => "刚刚".to_owned(),
        60..3600 => format!("{} 分钟前", seconds / 60),
        3600..86_400 => format!("{} 小时前", seconds / 3600),
        86_400..172_800 => "昨天".to_owned(),
        172_800..2_592_000 => format!("{} 天前", seconds / 86_400),
        2_592_000..31_536_000 => format!("{} 个月前", seconds / 2_592_000),
        _ => format!("{} 年前", seconds / 31_536_000),
    }
}

pub fn instance_meta(record: &InstanceRecord) -> String {
    format!("{} · {}", record.game_version, loader_label(record.loader))
}

pub fn library_card(record: &InstanceRecord, now: u64) -> LibraryCard {
    LibraryCard {
        id: record.id.clone(),
        name: record.name.clone(),
        meta: instance_meta(record),
        game_version: record.game_version.clone(),
        favorite: record.favorite,
        played: record.last_played.map_or_else(
            || "还没玩过".to_owned(),
            |at| format!("上次游玩 {}", relative_time(at, now)),
        ),
        seed: seed_of(&record.id),
        loader: record.loader,
        world: world_of(&record.id),
        created: record.created_at,
        source_project: record.source_project.clone(),
    }
}

/// Home's "needs attention" rows: each game's worst problem and what to do
/// first. Games no longer in the library are skipped.
#[must_use]
pub fn attention_rows(
    items: &[AttentionItem],
    library: &[LibraryCard],
) -> Vec<crate::home::AttentionRow> {
    items
        .iter()
        .filter_map(|item| {
            let card = library.iter().find(|card| card.id == item.instance_id)?;
            let (title, detail) = crate::instance_detail::problem_text(&item.headline);
            Some(crate::home::AttentionRow {
                instance: item.instance_id.clone(),
                name: card.name.clone(),
                title,
                detail,
                action: crate::instance_detail::problem_action(&item.headline.kind),
                more: item.total.saturating_sub(1),
            })
        })
        .collect()
}

/// Library order: favorites first, then most recently played, then newest.
pub fn library_cards(records: &[InstanceRecord], now: u64) -> Vec<LibraryCard> {
    let mut sorted: Vec<&InstanceRecord> = records.iter().collect();
    sorted.sort_by(|a, b| {
        b.favorite
            .cmp(&a.favorite)
            .then(b.last_played.cmp(&a.last_played))
            .then(b.created_at.cmp(&a.created_at))
    });
    sorted
        .into_iter()
        .map(|record| library_card(record, now))
        .collect()
}
