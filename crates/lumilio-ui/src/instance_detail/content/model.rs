use super::super::InstanceDetailView;
use gpui::{Context, Window, px};
use lumilio_core::{ContentEntry, ReleaseChannel};
use std::rc::Rc;

/// Which files the list shows.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ContentFilter {
    #[default]
    All,
    Updates,
    Disabled,
    Unknown,
}

pub(super) const FILTERS: [ContentFilter; 4] = [
    ContentFilter::All,
    ContentFilter::Updates,
    ContentFilter::Disabled,
    ContentFilter::Unknown,
];
pub(super) const FILTER_LABELS: [&str; 4] = ["全部", "有更新", "已停用", "未识别"];

/// What a bulk-bar button does to the view.
pub(super) type BulkAction =
    Rc<dyn Fn(&mut InstanceDetailView, &mut Window, &mut Context<InstanceDetailView>)>;

/// The toolbar and the bulk bar share one height (P-BULK).
pub(super) const BAR_HEIGHT: gpui::Pixels = px(40.);

/// The name a row shows: the Modrinth project, else the file.
#[must_use]
pub fn title_of(entry: &ContentEntry) -> &str {
    entry
        .source
        .as_ref()
        .map(|source| source.title.as_str())
        .filter(|title| !title.is_empty())
        .unwrap_or(&entry.item.display_name)
}

/// The rows a query and a filter leave, in list order.
#[must_use]
pub fn visible<'a>(
    entries: &'a [ContentEntry],
    query: &str,
    filter: ContentFilter,
) -> Vec<&'a ContentEntry> {
    let query = query.trim().to_lowercase();
    entries
        .iter()
        .filter(|entry| match filter {
            ContentFilter::All => true,
            ContentFilter::Updates => entry.update.is_some(),
            ContentFilter::Disabled => !entry.item.enabled,
            ContentFilter::Unknown => entry.source.is_none(),
        })
        .filter(|entry| {
            query.is_empty()
                || title_of(entry).to_lowercase().contains(&query)
                || entry.item.file_name.to_lowercase().contains(&query)
                || entry
                    .source
                    .as_ref()
                    .and_then(|source| source.author.as_ref())
                    .is_some_and(|author| author.to_lowercase().contains(&query))
        })
        .collect()
}

pub(super) const fn channel_label(channel: ReleaseChannel) -> &'static str {
    match channel {
        ReleaseChannel::Release => "正式",
        ReleaseChannel::Beta => "测试",
        ReleaseChannel::Alpha => "内测",
    }
}
