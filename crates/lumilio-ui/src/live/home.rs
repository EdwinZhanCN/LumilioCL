use super::library::{instance_meta, relative_time, world_of};
use crate::home::{HomePresentation, RecentEntry, Subject};
use lumilio_core::{HomeSummary, InstanceRecord};

/// Home from the library and its summary.
///
/// Continue is the summary's pick, or — before anything was ever played — the
/// newest instance, so the first launch is one press away. No instances at all
/// is first use. Returns the instance Continue would launch alongside.
pub fn home_presentation(
    records: &[InstanceRecord],
    summary: &HomeSummary,
    prefer: Option<&str>,
    now: u64,
) -> (HomePresentation, Option<String>) {
    let find = |id: &str| records.iter().find(|record| record.id == id);
    let subject_of = |record: &InstanceRecord| Subject {
        title: record.name.clone(),
        metadata: match record.last_played {
            Some(at) => format!(
                "{} · 上次游玩于 {}",
                instance_meta(record),
                relative_time(at, now)
            ),
            None => format!("{} · 还没玩过", instance_meta(record)),
        },
        world: world_of(&record.id),
    };
    // The current instance wins; without one, the latest played, then the newest.
    let chosen = prefer
        .and_then(find)
        .or_else(|| summary.continue_with.as_deref().and_then(find))
        .or_else(|| records.iter().max_by_key(|record| record.created_at));
    let Some(chosen) = chosen else {
        return (HomePresentation::FirstUse, None);
    };
    let recent = summary
        .recent
        .iter()
        .filter_map(|id| find(id))
        .filter(|record| record.id != chosen.id)
        .map(|record| RecentEntry {
            id: Some(record.id.clone()),
            title: record.name.clone(),
            metadata: format!(
                "{} · {}",
                instance_meta(record),
                record
                    .last_played
                    .map_or_else(String::new, |at| relative_time(at, now))
            ),
        })
        .collect();
    (
        HomePresentation::Continue {
            subject: subject_of(chosen),
            recent,
        },
        Some(chosen.id.clone()),
    )
}
