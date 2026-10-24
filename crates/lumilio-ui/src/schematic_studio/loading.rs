use std::rc::Rc;
use std::sync::Arc;

use gpui::Context;
use lumilio_plugin_api::FileInfo;

use super::{
    Analysis, Command, Connection, Event, Inspection, Loaded, Placement, StudioView, analyze, tree,
    writes,
};
use crate::toast::Toast;
use crate::tr;

impl StudioView {
    /// The application's answer to [`super::Load`]: start listing.
    pub fn connect(&mut self, connection: Connection, cx: &mut Context<Self>) {
        let receive = connection.receive;
        self.connection = Some(connection.send);
        self.listener = Some(cx.spawn(async move |this, cx| {
            while let Ok(event) = receive.recv().await {
                if this.update(cx, |this, cx| this.event(event, cx)).is_err() {
                    break;
                }
            }
        }));
        self.send(Command::List);
        self.want_history();
        self.send(Command::CanWrite);
        if self.placement == Placement::Window {
            self.send(Command::Names);
            self.send(Command::Schemes);
        }
        cx.notify();
    }

    pub(super) fn send(&self, command: Command) {
        if let Some(send) = &self.connection {
            let _ = send.try_send(command);
        }
    }

    /// The game started: until the application answers, nothing may write.
    pub fn write_access_changed(&mut self, cx: &mut Context<Self>) {
        self.can_write = false;
        self.send(Command::CanWrite);
        cx.notify();
    }

    /// The game ended and may have saved schematics: list again, and write
    /// actions follow whether the instance is free.
    pub fn files_changed(&mut self, cx: &mut Context<Self>) {
        self.send(Command::CanWrite);
        self.send(Command::List);
        cx.notify();
    }

    pub(crate) fn event(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Listed(Ok(files)) => {
                self.tree.set_files(&self.folder, files);
                self.listing = Loaded::Ready(());
                if let Some(selected) = &self.selected
                    && !self.tree.files().contains(selected)
                {
                    self.selected = None;
                    self.viewer = None;
                    self.inspection = Inspection::Empty;
                    self.opened = None;
                }
                let files = self.tree.files().to_vec();
                self.infos.retain(|file, _| files.contains(file));
                self.scan = self
                    .tree
                    .files()
                    .iter()
                    .filter(|file| !self.tree.has_fingerprint(file))
                    .cloned()
                    .collect();
                if let Some(file) = self.open_on_list.take()
                    && files.contains(&file)
                {
                    self.select_file(file, cx);
                }
                self.scan_next();
            }
            Event::Listed(Err(detail)) => self.listing = Loaded::Failed(detail),
            Event::Read { file, result } => self.read_arrived(file, result, cx),
            Event::History(result) => {
                self.history = match result {
                    Ok(entries) => Loaded::Ready(entries),
                    Err(detail) => Loaded::Failed(detail),
                };
            }
            Event::Backups(result) => {
                self.backups = match result {
                    Ok(backups) => Loaded::Ready(backups),
                    Err(detail) => Loaded::Failed(detail),
                };
            }
            Event::CanWrite(can) => self.can_write = can,
            Event::Applied { file, result } => self.applied(file, result, cx),
            Event::Removed(result) => self.removed(result),
            Event::Versions(result) => self.versions_arrived(result, cx),
            Event::Schemes(result) => self.schemes_arrived(result, cx),
            Event::Game(result) => {
                self.game = Some(match result {
                    Ok(version) => Loaded::Ready(version),
                    Err(detail) => Loaded::Failed(detail),
                });
                self.game_to_dialog(cx);
                self.want_compat(cx);
            }
            Event::Catalogs { target, result } => self.catalogs_arrived(target, result, cx),
            Event::PacksChanged(result) => self.packs_changed(result, cx),
            Event::SourceSwitched { path, on, result } => {
                self.source_switched(&path, on, result, cx);
            }
            Event::SourceMoved(result) => self.source_moved(result, cx),
            // Without names the studio writes IDs; nothing to report.
            Event::Names(Ok(names)) => self.names = Rc::new(names),
            Event::Names(Err(_)) => {}
            Event::Thumbnail { file, result } => self.thumbnail_arrived(file, result, cx),
            Event::Restored(result) => {
                self.restoring = false;
                match result {
                    Ok(entry) => {
                        self.toasts.push(Toast::success(tr!(
                            "studio-restored",
                            file = tree::stem(&entry.file)
                        )));
                        self.reread(&entry.file, cx);
                        self.send(Command::List);
                    }
                    Err(detail) => self
                        .toasts
                        .push(Toast::error(tr!("studio-restore-failed")).technical(detail)),
                }
                self.want_history();
                self.send(Command::CanWrite);
            }
        }
        cx.notify();
    }

    pub(super) fn scan_next(&mut self) {
        if self.scanning.is_some() {
            return;
        }
        while let Some(file) = self.scan.pop_front() {
            if !self.tree.has_fingerprint(&file) {
                self.send(Command::Read { file: file.clone() });
                self.scanning = Some(file);
                return;
            }
        }
    }

    pub(super) fn read_arrived(
        &mut self,
        file: String,
        result: Result<(Vec<u8>, FileInfo), String>,
        cx: &mut Context<Self>,
    ) {
        let inspect = self.selected.as_deref() == Some(file.as_str())
            && matches!(self.inspection, Inspection::Loading);
        if self.scanning.as_deref() == Some(file.as_str()) {
            self.scanning = None;
        }
        let bytes = match result {
            Ok((bytes, info)) => {
                self.infos.insert(file.clone(), info.clone());
                let bytes = Arc::new(bytes);
                if inspect {
                    self.opened = Some(writes::Opened {
                        file: file.clone(),
                        bytes: bytes.clone(),
                        info,
                    });
                }
                bytes
            }
            Err(detail) => {
                if inspect {
                    self.inspection = Inspection::Failed(detail);
                }
                self.scan_next();
                return;
            }
        };
        let work = cx
            .background_executor()
            .spawn(async move { analyze(&bytes, inspect) });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok((print, info, stats)) => {
                        // A write forgets the fingerprint, so an unknown
                        // one counts as a change too.
                        let changed = this.tree.fingerprint(&file) != Some(print);
                        this.tree.set_fingerprint(&file, print);
                        this.summaries.insert(file.clone(), info.clone());
                        this.want_thumbnail(&file, changed);
                        if let Some(stats) = stats
                            && this.selected.as_deref() == Some(file.as_str())
                        {
                            this.inspection = Inspection::Ready(Box::new(Analysis { stats, info }));
                            this.want_icons(cx);
                            this.want_compat(cx);
                            if let Some(id) = this.pending_block.take() {
                                this.choose_block(&id, cx);
                            }
                        }
                    }
                    Err(detail) => {
                        if inspect && this.selected.as_deref() == Some(file.as_str()) {
                            this.inspection = Inspection::Failed(detail);
                        }
                    }
                }
                this.scan_next();
                cx.notify();
            });
        })
        .detach();
    }
}
