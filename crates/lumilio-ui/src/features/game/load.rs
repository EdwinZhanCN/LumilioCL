//! Reading a game for its page: the record, and each section when the page
//! first shows it.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use gpui::{App, EntityId, WeakEntity};
use lumilio_core::{ContentEntry, ContentLookup, ProjectKind, ServiceError};

use super::content::load_pending;
use super::game_actions::GameWork;
use crate::instance_detail::{Arrived, InstanceDetailView, Section};

/// The newest read of each page's sections. A write that finishes while a
/// read is on its way reads again; the older read's answer is then dropped,
/// so it cannot put back what the write changed.
#[derive(Default)]
pub(super) struct SectionLoads {
    started: Cell<u64>,
    newest: RefCell<HashMap<(EntityId, Section), u64>>,
}

impl SectionLoads {
    fn start(&self, page: EntityId, section: Section) -> u64 {
        let load = self.started.get() + 1;
        self.started.set(load);
        self.newest.borrow_mut().insert((page, section), load);
        load
    }

    fn is_newest(&self, page: EntityId, section: Section, load: u64) -> bool {
        self.newest.borrow().get(&(page, section)) == Some(&load)
    }

    /// Whether `load` may show its answer; the newest one is forgotten then.
    fn finish(&self, page: EntityId, section: Section, load: u64) -> bool {
        let newest = self.is_newest(page, section, load);
        if newest {
            self.newest.borrow_mut().remove(&(page, section));
        }
        newest
    }
}

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

/// Reads one kind of content in steps, each shown as it comes: the files in
/// the folder, what the sources said about them before, then what the
/// sources say now, as far as `lookup` lets the earlier answers stand.
/// Asking the sources can take seconds.
pub(super) fn load_content(
    work: &GameWork,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    kind: ProjectKind,
    lookup: ContentLookup,
    cx: &mut App,
) {
    let section = Section::Content(kind);
    let page = view.entity_id();
    let load = work.loads().start(page, section);
    load_pending(work, id.clone(), view.clone(), cx);
    let service = work.service().clone();
    let files = work.spawn({
        let (service, id) = (service.clone(), id.clone());
        async move { service.content(&id, kind).await }
    });
    let kept = work.spawn({
        let (service, id) = (service.clone(), id.clone());
        async move {
            service
                .content_details(&id, kind, ContentLookup::Kept)
                .await
        }
    });
    let work = work.clone();
    cx.spawn(async move |cx| {
        // A folder that cannot be read is said by the last answer.
        if let Ok(Ok(items)) = files.await
            && work.loads().is_newest(page, section, load)
        {
            let entries = items
                .into_iter()
                .map(|item| ContentEntry {
                    item,
                    sha1: None,
                    source: None,
                    update: None,
                })
                .collect();
            let _ = view.update(cx, |view, cx| {
                view.arrived(Arrived::ContentSoFar(kind, entries), cx)
            });
        }
        if let Ok(Ok(list)) = kept.await
            && work.loads().is_newest(page, section, load)
        {
            let _ = view.update(cx, |view, cx| {
                view.arrived(Arrived::ContentSoFar(kind, list.entries), cx)
            });
        }
        // The files are hashed by now; only the sources are left to ask.
        if !work.loads().is_newest(page, section, load) {
            return;
        }
        let answer = work
            .spawn(async move { service.content_details(&id, kind, lookup).await })
            .await;
        let result = match answer {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        if work.loads().finish(page, section, load) {
            let _ = view.update(cx, |view, cx| {
                view.arrived(Arrived::Content(kind, result), cx)
            });
        }
    })
    .detach();
}

/// Reads one section's data off the interface thread; content in steps
/// (see [`load_content`]).
pub(super) fn load_section(
    work: &GameWork,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    section: Section,
    cx: &mut App,
) {
    if let Section::Content(kind) = section {
        load_content(work, id, view, kind, ContentLookup::Fresh, cx);
        return;
    }
    let page = view.entity_id();
    let load = work.loads().start(page, section);
    let service = work.service().clone();
    let handle = work.spawn(async move {
        match section {
            // Read in steps by `load_content`; in one answer here.
            Section::Content(kind) => Arrived::Content(
                kind,
                service
                    .content_details(&id, kind, ContentLookup::Fresh)
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
    let work = work.clone();
    cx.spawn(async move |cx| {
        let arrived = match handle.await {
            Ok(arrived) => arrived,
            Err(error) => failed_section(section, error.to_string()),
        };
        if work.loads().finish(page, section, load) {
            let _ = view.update(cx, |view, cx| view.arrived(arrived, cx));
        }
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
