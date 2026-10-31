use gpui::{App, AppContext as _, WeakEntity, Window};
use lumilio_core::{ContentLookup, ServiceError};

use super::content::{load_changelog, load_versions, pick_pending_files};
use super::files::{
    copy_screenshot, export_log, make_thumbnail, open_crash, open_export, open_folder,
    open_runtime_change, ping_server,
};
use super::game_actions::{GameEvent, GameWork};
use super::load::{load_content, load_instance, load_section};
use super::plugins::{
    open_studio_window, plugin_action, plugin_map, plugin_model, plugin_studio, plugin_tabs,
    plugin_view, pop_out_map,
};
use super::write::write_instance;
use super::{logs, options};
use crate::instance_detail::{InstanceDetailView, InstanceIntent};
use crate::{platform, tr};

/// Answers what the page of game `id` asks.
pub(super) fn answer(
    work: &GameWork,
    id: &str,
    view: WeakEntity<InstanceDetailView>,
    intent: InstanceIntent,
    window: &mut Window,
    cx: &mut App,
) {
    match intent {
        // The shell answers these: they move the window, choose the current
        // game, ask the page a question, or play and stop (the application's).
        InstanceIntent::Play
        | InstanceIntent::PlayWorld(_)
        | InstanceIntent::PlayServer(_)
        | InstanceIntent::Stop
        | InstanceIntent::OpenAccounts
        | InstanceIntent::SetCurrent
        | InstanceIntent::OpenProject { .. }
        | InstanceIntent::BrowseContent(_)
        | InstanceIntent::AskCopy
        | InstanceIntent::AskDelete
        | InstanceIntent::Resolve(_) => {}
        InstanceIntent::ChoosePreset => {
            options::choose_preset(work, id.to_owned(), view, window, cx)
        }
        InstanceIntent::EditOption(key) => {
            options::edit_option(work, id.to_owned(), key, view, window, cx)
        }
        write @ (InstanceIntent::SetOption { .. }
        | InstanceIntent::ResolveOptions { .. }
        | InstanceIntent::RecoverOptions) => {
            options::write_option(work, id.to_owned(), view, write, cx)
        }
        InstanceIntent::OpenRuntimeChange => {
            open_runtime_change(work, id.to_owned(), view, window, cx)
        }
        InstanceIntent::PingServer(address) => ping_server(work, id.to_owned(), view, address, cx),
        InstanceIntent::Thumbnail(file) => make_thumbnail(work, id.to_owned(), view, file, cx),
        InstanceIntent::CopyScreenshot(file) => {
            copy_screenshot(work, id.to_owned(), view, file, cx)
        }
        InstanceIntent::PluginTabs => plugin_tabs(work, id.to_owned(), view, cx),
        InstanceIntent::PluginView(plugin) => plugin_view(work, id.to_owned(), view, plugin, cx),
        InstanceIntent::LoadMap { request } => plugin_map(work, id.to_owned(), view, request, cx),
        InstanceIntent::LoadStudio {
            plugin,
            folder,
            request,
        } => plugin_studio(work, id.to_owned(), view, plugin, folder, request, cx),
        InstanceIntent::PopOutMap { handoff } => pop_out_map(work, id.to_owned(), handoff, cx),
        InstanceIntent::OpenStudioWindow {
            plugin,
            folder,
            file,
        } => open_studio_window(work, id.to_owned(), plugin, folder, file, cx),
        InstanceIntent::LoadModel {
            plugin,
            file,
            request,
        } => plugin_model(work, id.to_owned(), view, plugin, file, request, cx),
        InstanceIntent::PluginAction { plugin, action } => {
            plugin_action(work, id.to_owned(), view, plugin, action, cx)
        }
        InstanceIntent::Reload => load_instance(work, id.to_owned(), view, cx),
        InstanceIntent::OpenCrash(file) => open_crash(work, id.to_owned(), view, file, cx),
        InstanceIntent::OpenGameLog(source) => logs::read(work, id.to_owned(), view, source, cx),
        InstanceIntent::AnalyzeGameLog {
            request,
            text,
            crash,
        } => logs::analyze(work, id.to_owned(), view, request, text, crash, cx),
        InstanceIntent::ExportGameLog { source, live } => {
            logs::export(work, id.to_owned(), source, live, cx)
        }
        InstanceIntent::Load(section) => load_section(work, id.to_owned(), view, section, cx),
        InstanceIntent::IdentifyContent(kind) => {
            load_content(work, id.to_owned(), view, kind, ContentLookup::Again, cx)
        }
        InstanceIntent::ExportLog(crash) => export_log(work, id.to_owned(), crash, cx),
        InstanceIntent::OpenFolder(folder) => open_folder(work, id.to_owned(), view, folder, cx),
        InstanceIntent::RevealPath(relative) => {
            // Only plain names below the game directory; never a way out of it.
            let plain = std::path::Path::new(&relative)
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)));
            if plain {
                cx.reveal_path(&work.service().game_dir(id).join(relative));
            }
        }
        InstanceIntent::LoadVersions { source, project } => {
            load_versions(work, view, source, project, cx)
        }
        InstanceIntent::LoadChangelog {
            source,
            project,
            version,
        } => load_changelog(work, view, source, project, version, cx),
        InstanceIntent::AddPendingFiles => pick_pending_files(work, id, view, window, cx),
        InstanceIntent::RevealContent { kind, file_name } => {
            let folder = kind.install_folder().unwrap_or_default();
            let path = work.service().game_dir(id).join(folder).join(file_name);
            cx.reveal_path(&path);
        }
        InstanceIntent::ImportContent(kind) => {
            let picked = cx.prompt_for_paths(gpui::PathPromptOptions {
                files: true,
                directories: false,
                multiple: true,
                prompt: Some(tr!("instance-import-content-prompt").into()),
            });
            let work = work.clone();
            let id = id.to_owned();
            let handle = window.window_handle();
            cx.spawn(async move |cx| {
                let Ok(Ok(Some(files))) = picked.await else {
                    return;
                };
                let _ = cx.update_window(handle, |_, window, cx| {
                    write_instance(
                        &work,
                        id,
                        view,
                        InstanceIntent::AddFiles { kind, files },
                        window,
                        cx,
                    );
                });
            })
            .detach();
        }
        InstanceIntent::ExportWorld(folder) => {
            let start = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| work.service().layout().root().to_owned());
            let chosen = platform::pick_save_path(cx, &start, &format!("{folder}.zip"));
            let work = work.clone();
            let id = id.to_owned();
            let handle = window.window_handle();
            cx.spawn(async move |cx| {
                let Some(path) = chosen.await else {
                    return;
                };
                let _ = cx.update_window(handle, |_, window, cx| {
                    write_instance(
                        &work,
                        id,
                        view,
                        InstanceIntent::ExportWorldTo { folder, path },
                        window,
                        cx,
                    );
                });
            })
            .detach();
        }
        InstanceIntent::BackupGame => {
            let start = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| work.service().layout().root().to_owned());
            let suggested = format!("{}.lumilio-backup.zip", id).replace('/', "-");
            let chosen = platform::pick_save_path(cx, &start, &suggested);
            let work = work.clone();
            let id = id.to_owned();
            let handle = window.window_handle();
            cx.spawn(async move |cx| {
                let Some(path) = chosen.await else {
                    return;
                };
                let _ = cx.update_window(handle, |_, window, cx| {
                    write_instance(
                        &work,
                        id,
                        view,
                        InstanceIntent::BackupGameTo(path),
                        window,
                        cx,
                    );
                });
            })
            .detach();
        }
        InstanceIntent::ExportPack => open_export(work, id.to_owned(), view, window, cx),
        InstanceIntent::ExportPackAs(spec) => {
            let start = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| work.service().layout().root().to_owned());
            let suggested = format!("{}-{}.{}", spec.name, spec.version, spec.format.extension())
                .replace('/', "-");
            let chosen = platform::pick_save_path(cx, &start, &suggested);
            let work = work.clone();
            let id = id.to_owned();
            let handle = window.window_handle();
            cx.spawn(async move |cx| {
                let Some(path) = chosen.await else {
                    return;
                };
                let _ = cx.update_window(handle, |_, window, cx| {
                    write_instance(
                        &work,
                        id,
                        view,
                        InstanceIntent::ExportPackTo { spec, path },
                        window,
                        cx,
                    );
                });
            })
            .detach();
        }
        InstanceIntent::ImportWorld => {
            let picked = cx.prompt_for_paths(gpui::PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some(tr!("instance-import-world-prompt").into()),
            });
            let work = work.clone();
            let id = id.to_owned();
            let handle = window.window_handle();
            cx.spawn(async move |cx| {
                let Ok(Ok(Some(files))) = picked.await else {
                    return;
                };
                let Some(path) = files.into_iter().next() else {
                    return;
                };
                let _ = cx.update_window(handle, |_, window, cx| {
                    write_instance(&work, id, view, InstanceIntent::AddWorld(path), window, cx);
                });
            })
            .detach();
        }
        InstanceIntent::Install
        | InstanceIntent::Repair
        | InstanceIntent::NewGameFolder
        | InstanceIntent::ChangeRuntime { .. }
        | InstanceIntent::Copy { .. }
        | InstanceIntent::Delete
        | InstanceIntent::SetContent { .. }
        | InstanceIntent::DeleteContent { .. }
        | InstanceIntent::CopyWorld(_)
        | InstanceIntent::BackupWorld(_)
        | InstanceIntent::ExportWorldTo { .. }
        | InstanceIntent::ExportPackTo { .. }
        | InstanceIntent::AddWorld(_)
        | InstanceIntent::DeleteWorld(_)
        | InstanceIntent::DeleteScreenshot(_)
        | InstanceIntent::SaveServer { .. }
        | InstanceIntent::DeleteServer { .. }
        | InstanceIntent::MoveServer { .. }
        | InstanceIntent::InstallJava(_)
        | InstanceIntent::BackupGameTo(_)
        | InstanceIntent::CreateSnapshot
        | InstanceIntent::CreateSnapshotAs { .. }
        | InstanceIntent::RestoreSnapshot(_)
        | InstanceIntent::DeleteSnapshot(_)
        | InstanceIntent::SwitchContent { .. }
        | InstanceIntent::UpdateContent { .. }
        | InstanceIntent::PendingFilesChosen(_)
        | InstanceIntent::AddFiles { .. } => {
            write_instance(work, id.to_owned(), view, intent, window, cx)
        }
        action @ (InstanceIntent::Rename(_)
        | InstanceIntent::SaveMemory(_)
        | InstanceIntent::SaveSettings(_)) => {
            let service = work.service().clone();
            let id = id.to_owned();
            let memory = matches!(action, InstanceIntent::SaveMemory(_));
            let settings_dialog = matches!(action, InstanceIntent::SaveSettings(_));
            let handle = work.spawn(async move {
                match action {
                    InstanceIntent::Rename(name) => service.rename(&id, &name).await?,
                    InstanceIntent::SaveMemory(settings)
                    | InstanceIntent::SaveSettings(settings) => {
                        service.update_instance_settings(&id, settings).await?
                    }
                    _ => unreachable!("only save intents reach this branch"),
                }
                Ok::<_, ServiceError>((service.instance(&id).await?, service.settings().await))
            });
            let work = work.clone();
            cx.spawn(async move |cx| {
                let result = match handle.await {
                    Ok(result) => result.map_err(|error| error.to_string()),
                    Err(error) => Err(error.to_string()),
                };
                let _ = view.update(cx, |view, cx| {
                    if settings_dialog {
                        view.settings_saved(result, cx);
                    } else {
                        view.saved(memory, result, cx);
                    }
                });
                work.emit(cx, GameEvent::ListsChanged);
            })
            .detach();
        }
    }
}
