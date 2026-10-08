use super::instance::{ContentAction, content_notice, load_instance, load_section};
use super::jobs::reload;
use super::{Reload, Wiring};
use gpui_kit::{App, WeakEntity, Window};
use lumilio_core::{CancellationToken, ServiceError, SnapshotScope};
use lumilio_ui::instance_detail::{
    InstanceDetailView, InstanceIntent, Operated, Section, export_notice,
};
use lumilio_ui::tr;

/// One write on an instance: runs it, tells the view, and refreshes what it
/// made stale. Failures keep the screen as it was and say what to do next.
pub(super) fn write_instance(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    intent: InstanceIntent,
    _window: &mut Window,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let cancel = CancellationToken::new();
    let deleting = matches!(intent, InstanceIntent::Delete);
    let installing = matches!(
        intent,
        InstanceIntent::Install | InstanceIntent::Repair | InstanceIntent::ChangeRuntime { .. }
    );
    let work = {
        let id = id.clone();
        async move {
            let done = |notice: &str, refresh: Vec<Section>| Ok((notice.to_owned(), refresh));
            match intent {
                InstanceIntent::SetContent {
                    kind,
                    files,
                    enabled,
                } => {
                    let results = service
                        .set_content_state(&id, kind, &files, enabled)
                        .await?;
                    let action = if enabled {
                        ContentAction::Enable
                    } else {
                        ContentAction::Disable
                    };
                    match content_notice(&results, action) {
                        Ok(text) => done(&text, vec![Section::Content(kind)]),
                        Err(detail) => Err(ServiceError::Remote(detail)),
                    }
                }
                InstanceIntent::DeleteContent { kind, files } => {
                    let results = service.delete_content(&id, kind, &files).await?;
                    match content_notice(&results, ContentAction::Delete) {
                        Ok(text) => done(&text, vec![Section::Content(kind)]),
                        Err(detail) => Err(ServiceError::Remote(detail)),
                    }
                }
                InstanceIntent::SwitchContent {
                    kind,
                    file_name,
                    project,
                    version_id,
                } => {
                    let name = service
                        .switch_content_version(
                            &id,
                            kind,
                            &file_name,
                            &project,
                            &version_id,
                            cancel,
                        )
                        .await?;
                    done(
                        &tr!("instance-content-switched", name = name.as_str()),
                        vec![Section::Content(kind)],
                    )
                }
                InstanceIntent::UpdateContent { kind, updates } => {
                    // One by one: each file stands or falls on its own.
                    let total = updates.len();
                    let mut failed = Vec::new();
                    for (file, project, version) in updates {
                        if let Err(error) = service
                            .switch_content_version(
                                &id,
                                kind,
                                &file,
                                &project,
                                &version,
                                cancel.clone(),
                            )
                            .await
                        {
                            failed.push(format!("{file}: {error}"));
                        }
                    }
                    if failed.is_empty() {
                        done(
                            &tr!("instance-content-updated", count = total),
                            vec![Section::Content(kind)],
                        )
                    } else {
                        Err(ServiceError::Remote(tr!(
                            "instance-content-update-partial",
                            ok = total - failed.len(),
                            failed = failed.len(),
                            detail = failed.join("\n")
                        )))
                    }
                }
                InstanceIntent::AddFiles { kind, files } => {
                    let results = service.import_content(&id, kind, files).await?;
                    let added = results.iter().filter(|(_, result)| result.is_ok()).count();
                    let refused: Vec<String> = results
                        .iter()
                        .filter_map(|(name, result)| {
                            result
                                .as_ref()
                                .err()
                                .map(|error| format!("{name}: {error}"))
                        })
                        .collect();
                    if refused.is_empty() {
                        done(
                            &tr!("instance-content-added", count = added),
                            vec![Section::Content(kind)],
                        )
                    } else {
                        Err(ServiceError::Remote(tr!(
                            "instance-content-add-partial",
                            ok = added,
                            failed = refused.len(),
                            detail = refused.join("\n")
                        )))
                    }
                }
                InstanceIntent::CopyWorld(folder) => {
                    let name = service.copy_world(&id, &folder, None).await?;
                    done(
                        &tr!("instance-world-copied", name = name.as_str()),
                        vec![Section::Worlds],
                    )
                }
                InstanceIntent::BackupWorld(folder) => {
                    service
                        .create_snapshot(
                            &id,
                            SnapshotScope::World(folder.clone()),
                            tr!("instance-snapshot-manual-backup"),
                        )
                        .await?;
                    done(
                        &tr!("instance-world-backed-up", name = folder.as_str()),
                        vec![Section::Snapshots],
                    )
                }
                InstanceIntent::ExportWorldTo { folder, path } => {
                    service.export_world(&id, &folder, &path).await?;
                    done(
                        &tr!("instance-exported-to", path = path.display().to_string()),
                        Vec::new(),
                    )
                }
                InstanceIntent::ExportPackTo { spec, path } => {
                    let report = service.export_modpack(&id, spec, &path, cancel).await?;
                    done(&export_notice(&report, &path), Vec::new())
                }
                InstanceIntent::AddWorld(path) => {
                    let name = service.import_world(&id, &path).await?;
                    done(
                        &tr!("instance-world-imported", name = name.as_str()),
                        vec![Section::Worlds],
                    )
                }
                InstanceIntent::DeleteWorld(folder) => {
                    service.delete_world(&id, &folder).await?;
                    done(tr!("instance-world-deleted"), vec![Section::Worlds])
                }
                InstanceIntent::SaveServer {
                    index,
                    expected,
                    entry,
                } => {
                    let name = entry.name.clone();
                    match (index, expected) {
                        (Some(index), Some(expected)) => {
                            service.update_server(&id, index, expected, entry).await?;
                            done(
                                &tr!("instance-server-saved", name = name.as_str()),
                                vec![Section::Servers],
                            )
                        }
                        _ => {
                            service.add_server(&id, entry).await?;
                            done(
                                &tr!("instance-server-added", name = name.as_str()),
                                vec![Section::Servers],
                            )
                        }
                    }
                }
                InstanceIntent::DeleteScreenshot(file) => {
                    service.delete_screenshot(&id, &file).await?;
                    done(
                        tr!("instance-screenshot-deleted"),
                        vec![Section::Screenshots],
                    )
                }
                InstanceIntent::DeleteServer { index, expected } => {
                    service.remove_server(&id, index, expected).await?;
                    done(tr!("instance-server-deleted"), vec![Section::Servers])
                }
                InstanceIntent::MoveServer {
                    index,
                    expected,
                    to,
                } => {
                    service.move_server(&id, index, expected, to).await?;
                    done(tr!("instance-servers-reordered"), vec![Section::Servers])
                }
                InstanceIntent::CreateSnapshot => {
                    service
                        .create_snapshot(&id, SnapshotScope::Full, tr!("instance-snapshot-manual"))
                        .await?;
                    done(tr!("instance-snapshot-created"), vec![Section::Snapshots])
                }
                InstanceIntent::CreateSnapshotAs { note, world } => {
                    let (scope, label) = match world {
                        Some(folder) => (
                            SnapshotScope::World(folder),
                            if note.is_empty() {
                                tr!("instance-snapshot-manual-backup").to_owned()
                            } else {
                                note
                            },
                        ),
                        None => (
                            SnapshotScope::Full,
                            if note.is_empty() {
                                tr!("instance-snapshot-manual").to_owned()
                            } else {
                                note
                            },
                        ),
                    };
                    service.create_snapshot(&id, scope, &label).await?;
                    done(tr!("instance-snapshot-created"), vec![Section::Snapshots])
                }
                InstanceIntent::RestoreSnapshot(snapshot) => {
                    service.restore_snapshot(&id, &snapshot).await?;
                    done(
                        tr!("instance-snapshot-restored"),
                        vec![Section::Worlds, Section::Snapshots],
                    )
                }
                InstanceIntent::DeleteSnapshot(snapshot) => {
                    service.delete_snapshot(&id, &snapshot).await?;
                    done(tr!("instance-snapshot-deleted"), vec![Section::Snapshots])
                }
                InstanceIntent::Install => {
                    // Progress lives in Activity; the channel only has to drain.
                    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                    tokio::spawn(async move { while rx.recv().await.is_some() {} });
                    service.install_instance(&id, tx, cancel).await?;
                    done(tr!("instance-files-installed"), Vec::new())
                }
                InstanceIntent::BackupGameTo(path) => {
                    service.backup_instance(&id, &path, cancel).await?;
                    done(
                        &tr!("instance-backed-up-to", path = path.display().to_string()),
                        Vec::new(),
                    )
                }
                InstanceIntent::InstallJava(major) => {
                    service.install_java(major, cancel).await?;
                    done(tr!("instance-java-installed"), vec![Section::Problems])
                }
                InstanceIntent::Repair => {
                    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                    tokio::spawn(async move { while rx.recv().await.is_some() {} });
                    service.repair_instance(&id, tx, cancel).await?;
                    done(tr!("instance-files-repaired"), Vec::new())
                }
                InstanceIntent::ChangeRuntime {
                    loader,
                    game_version,
                    loader_version,
                } => {
                    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                    tokio::spawn(async move { while rx.recv().await.is_some() {} });
                    let record = service
                        .change_runtime(
                            &id,
                            &game_version,
                            loader,
                            loader_version.as_deref(),
                            tx,
                            cancel,
                        )
                        .await?;
                    done(
                        &tr!(
                            "instance-runtime-changed",
                            runtime = lumilio_ui::live::instance_meta(&record)
                        ),
                        Vec::new(),
                    )
                }
                InstanceIntent::Copy {
                    name,
                    include_worlds,
                } => {
                    let record = service
                        .copy_instance(&id, &name, include_worlds, cancel)
                        .await?;
                    done(
                        &tr!("instance-game-copied", name = record.name.as_str()),
                        Vec::new(),
                    )
                }
                InstanceIntent::Delete => {
                    service.delete_instance(&id).await?;
                    done(tr!("instance-game-deleted"), Vec::new())
                }
                _ => unreachable!("only writes reach this function"),
            }
        }
    };
    let handle = wiring
        .backend
        .spawn(async move { work.await as Result<(String, Vec<Section>), ServiceError> });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let succeeded = result.is_ok();
        let done = match result {
            Ok((notice, refresh)) => Operated {
                notice,
                technical: None,
                refresh,
            },
            Err(detail) => Operated {
                notice: tr!("instance-write-failed").into(),
                technical: Some(detail),
                refresh: Vec::new(),
            },
        };
        let stale = view
            .update(cx, |view, cx| view.operated(done, cx))
            .unwrap_or_default();
        cx.update(|cx| {
            for section in stale {
                load_section(&wiring, id.clone(), view.clone(), section, cx);
            }
            if succeeded && installing {
                load_instance(&wiring, id.clone(), view.clone(), cx);
            }
            if succeeded && deleting {
                let shell = wiring.shell.clone();
                let _ = shell.update(cx, |shell, cx| shell.forget_instance(&id, cx));
            }
            reload(&wiring, Reload::All, cx);
        });
    })
    .detach();
}
