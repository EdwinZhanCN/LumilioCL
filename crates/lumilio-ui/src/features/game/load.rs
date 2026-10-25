//! Reading a game for its page: the record, and each section when the page
//! first shows it.

use gpui::{App, WeakEntity};
use lumilio_core::ServiceError;

use super::content::load_pending;
use super::game_actions::GameWork;
use crate::instance_detail::{Arrived, InstanceDetailView, Section};

/// Reads the game's record and the machine's memory for its page.
pub(super) fn load_instance(
    work: &GameWork,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    cx: &mut App,
) {
    let service = work.service().clone();
    let handle = work.spawn(async move {
        let record = service.instance(&id).await?;
        Ok::<_, ServiceError>((record, service.settings().await))
    });
    let memory = work.spawn(async { lumilio_core::total_memory_mb() });
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let _ = view.update(cx, |view, cx| view.loaded(result, cx));
        if let Ok(total) = memory.await {
            let _ = view.update(cx, |view, cx| view.set_machine_memory(total, cx));
        }
    })
    .detach();
}

/// Reads one section's data off the interface thread.
pub(super) fn load_section(
    work: &GameWork,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    section: Section,
    cx: &mut App,
) {
    if matches!(section, Section::Content(_)) {
        load_pending(work, id.clone(), view.clone(), cx);
    }
    let service = work.service().clone();
    let handle = work.spawn(async move {
        match section {
            Section::Content(kind) => Arrived::Content(
                kind,
                service
                    .content_details(&id, kind)
                    .await
                    .map_err(|error| error.to_string()),
            ),
            Section::Worlds => {
                Arrived::Worlds(service.worlds(&id).await.map_err(|error| error.to_string()))
            }
            Section::Servers => Arrived::Servers(
                service
                    .servers(&id)
                    .await
                    .map_err(|error| error.to_string()),
            ),
            Section::Screenshots => Arrived::Screenshots(
                service
                    .screenshots(&id)
                    .await
                    .map_err(|error| error.to_string()),
            ),
            Section::Snapshots => Arrived::Snapshots(
                service
                    .snapshots(&id)
                    .await
                    .map_err(|error| error.to_string()),
            ),
            Section::History => Arrived::History(
                service
                    .history(&id)
                    .await
                    .map_err(|error| error.to_string()),
            ),
            Section::Logs => {
                Arrived::Logs(service.logs(&id).await.map_err(|error| error.to_string()))
            }
            Section::Problems => Arrived::Problems(
                service
                    .problems(&id)
                    .await
                    .map_err(|error| error.to_string()),
            ),
            Section::Files => files_arrived(&service, &id, String::new()).await,
            Section::Size => Arrived::Size(
                service
                    .instance_size(&id)
                    .await
                    .map_err(|error| error.to_string()),
            ),
            Section::GameOptions => Arrived::GameOptions(
                service
                    .game_options(&id)
                    .await
                    .map(Box::new)
                    .map_err(|error| error.to_string()),
            ),
            Section::Java => Arrived::Java(
                service
                    .instance_java(&id)
                    .await
                    .map(Box::new)
                    .map_err(|error| error.to_string()),
            ),
        }
    });
    cx.spawn(async move |cx| {
        let arrived = match handle.await {
            Ok(arrived) => arrived,
            Err(error) => failed_section(section, error.to_string()),
        };
        let _ = view.update(cx, |view, cx| view.arrived(arrived, cx));
    })
    .detach();
}

pub(super) fn failed_section(section: Section, detail: String) -> Arrived {
    match section {
        Section::Content(kind) => Arrived::Content(kind, Err(detail)),
        Section::Worlds => Arrived::Worlds(Err(detail)),
        Section::Servers => Arrived::Servers(Err(detail)),
        Section::Screenshots => Arrived::Screenshots(Err(detail)),
        Section::Snapshots => Arrived::Snapshots(Err(detail)),
        Section::History => Arrived::History(Err(detail)),
        Section::Logs => Arrived::Logs(Err(detail)),
        Section::Problems => Arrived::Problems(Err(detail)),
        Section::Files => Arrived::Files(String::new(), Err(detail)),
        Section::Size => Arrived::Size(Err(detail)),
        Section::GameOptions => Arrived::GameOptions(Err(detail)),
        Section::Java => Arrived::Java(Err(detail)),
    }
}

/// One folder of the game directory, as the Files part shows it.
pub(super) async fn files_arrived(
    service: &crate::runtime::Service,
    id: &str,
    folder: String,
) -> Arrived {
    let result = service
        .list_files(id, &folder)
        .await
        .map_err(|error| error.to_string());
    Arrived::Files(folder, result)
}
