use super::instance::{content_notice, load_instance, load_section};
use super::jobs::reload;
use super::{Reload, Wiring};
use gpui_kit::{App, WeakEntity, Window};
use lumilio_core::{CancellationToken, ServiceError, SnapshotScope};
use lumilio_ui::instance_detail::{
    InstanceDetailView, InstanceIntent, Operated, Section, export_notice,
};

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
                    let did = if enabled { "启用" } else { "停用" };
                    match content_notice(&results, did) {
                        Ok(text) => done(&text, vec![Section::Content(kind)]),
                        Err(detail) => Err(ServiceError::Remote(detail)),
                    }
                }
                InstanceIntent::DeleteContent { kind, files } => {
                    let results = service.delete_content(&id, kind, &files).await?;
                    match content_notice(&results, "删除") {
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
                    done(&format!("已换成 {name}"), vec![Section::Content(kind)])
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
                            &format!("已更新 {total} 个文件"),
                            vec![Section::Content(kind)],
                        )
                    } else {
                        Err(ServiceError::Remote(format!(
                            "{} 个更新成功，{} 个没有成功\n{}",
                            total - failed.len(),
                            failed.len(),
                            failed.join("\n")
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
                            &format!("已添加 {added} 个文件"),
                            vec![Section::Content(kind)],
                        )
                    } else {
                        Err(ServiceError::Remote(format!(
                            "添加了 {added} 个，{} 个没有添加\n{}",
                            refused.len(),
                            refused.join("\n")
                        )))
                    }
                }
                InstanceIntent::CopyWorld(folder) => {
                    let name = service.copy_world(&id, &folder, None).await?;
                    done(&format!("已复制为「{name}」"), vec![Section::Worlds])
                }
                InstanceIntent::BackupWorld(folder) => {
                    service
                        .create_snapshot(&id, SnapshotScope::World(folder.clone()), "手动备份")
                        .await?;
                    done(
                        &format!("已备份「{folder}」，可以在历史的快照里恢复"),
                        vec![Section::Snapshots],
                    )
                }
                InstanceIntent::ExportWorldTo { folder, path } => {
                    service.export_world(&id, &folder, &path).await?;
                    done(&format!("已导出到 {}", path.display()), Vec::new())
                }
                InstanceIntent::ExportPackTo { spec, path } => {
                    let report = service.export_modpack(&id, spec, &path, cancel).await?;
                    done(&export_notice(&report, &path), Vec::new())
                }
                InstanceIntent::AddWorld(path) => {
                    let name = service.import_world(&id, &path).await?;
                    done(&format!("已导入「{name}」"), vec![Section::Worlds])
                }
                InstanceIntent::DeleteWorld(folder) => {
                    service.delete_world(&id, &folder).await?;
                    done("世界已删除", vec![Section::Worlds])
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
                            done(&format!("已保存「{name}」"), vec![Section::Servers])
                        }
                        _ => {
                            service.add_server(&id, entry).await?;
                            done(&format!("已添加「{name}」"), vec![Section::Servers])
                        }
                    }
                }
                InstanceIntent::DeleteServer { index, expected } => {
                    service.remove_server(&id, index, expected).await?;
                    done("服务器已删除", vec![Section::Servers])
                }
                InstanceIntent::MoveServer {
                    index,
                    expected,
                    to,
                } => {
                    service.move_server(&id, index, expected, to).await?;
                    done("已调整顺序", vec![Section::Servers])
                }
                InstanceIntent::CreateSnapshot => {
                    service
                        .create_snapshot(&id, SnapshotScope::Full, "手动快照")
                        .await?;
                    done("快照已创建", vec![Section::Snapshots])
                }
                InstanceIntent::CreateSnapshotAs { note, world } => {
                    let (scope, label) = match world {
                        Some(folder) => (
                            SnapshotScope::World(folder),
                            if note.is_empty() {
                                "手动备份".to_owned()
                            } else {
                                note
                            },
                        ),
                        None => (
                            SnapshotScope::Full,
                            if note.is_empty() {
                                "手动快照".to_owned()
                            } else {
                                note
                            },
                        ),
                    };
                    service.create_snapshot(&id, scope, &label).await?;
                    done("快照已创建", vec![Section::Snapshots])
                }
                InstanceIntent::RestoreSnapshot(snapshot) => {
                    service.restore_snapshot(&id, &snapshot).await?;
                    done("已恢复快照", vec![Section::Worlds, Section::Snapshots])
                }
                InstanceIntent::DeleteSnapshot(snapshot) => {
                    service.delete_snapshot(&id, &snapshot).await?;
                    done("快照已删除", vec![Section::Snapshots])
                }
                InstanceIntent::Install => {
                    // Progress lives in Activity; the channel only has to drain.
                    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                    tokio::spawn(async move { while rx.recv().await.is_some() {} });
                    service.install_instance(&id, tx, cancel).await?;
                    done("游戏文件已安装", Vec::new())
                }
                InstanceIntent::BackupGameTo(path) => {
                    service.backup_instance(&id, &path, cancel).await?;
                    done(&format!("已备份到 {}", path.display()), Vec::new())
                }
                InstanceIntent::InstallJava(major) => {
                    service.install_java(major, cancel).await?;
                    done("Java 已安装", vec![Section::Problems])
                }
                InstanceIntent::Repair => {
                    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                    tokio::spawn(async move { while rx.recv().await.is_some() {} });
                    service.repair_instance(&id, tx, cancel).await?;
                    done("游戏文件已检查，缺的和损坏的已补上", Vec::new())
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
                        &format!("已更换为 {}", lumilio_ui::live::instance_meta(&record)),
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
                    done(&format!("已复制为「{}」", record.name), Vec::new())
                }
                InstanceIntent::Delete => {
                    service.delete_instance(&id).await?;
                    done("游戏已删除", Vec::new())
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
                notice: "没有成功，游戏保持原样，可以稍后重试".into(),
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
