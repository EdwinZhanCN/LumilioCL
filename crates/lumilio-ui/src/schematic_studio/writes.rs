//! The studio's writes (plan schematic-studio, D8/D9, T13–T16): edited bytes
//! are made on the background executor from the chosen file as it was read,
//! then sent with what the file looked like, so the application writes only
//! a file nobody changed since. Deleting and "keep only this" move files into
//! the launcher's backups; nothing is ever deleted.

use std::sync::Arc;

use gpui::{App, Context, Entity, Window};
use gpui_component::WindowExt as _;
use lumilio_core::{BlockCatalog, SchematicLogEntry, SchematicOperation};
use lumilio_plugin_api::FileInfo;

use super::dialogs::{OpenDialog, Submission};
use super::{Command, Inspection, Loaded, StudioView, WriteFailure, tree};
use crate::toast::Toast;
use crate::tr;

/// The `operation` of a deletion's log line.
pub(super) const DELETE: &str = "delete";
/// The `operation` of a "keep only this" line.
pub(super) const DEDUPE: &str = "dedupe";

/// The chosen file as it was last read.
pub(crate) struct Opened {
    pub(crate) file: String,
    pub(crate) bytes: Arc<Vec<u8>>,
    pub(crate) info: FileInfo,
}

/// What a background edit made: the new bytes and how many things changed.
pub(super) struct Edited {
    pub(super) bytes: Vec<u8>,
    pub(super) count: u64,
}

/// Turns the changed count into the log line and the success toast, on the
/// interface thread (the words follow the current language).
pub(super) type Describe = Box<dyn FnOnce(u64) -> (SchematicOperation, String)>;

/// A write that was sent from a dialog and waits for its answer.
pub(crate) struct Pending {
    status: Entity<Submission>,
    done: String,
    /// The new file a save beside the chosen one writes, chosen once it is
    /// there (T23).
    select: Option<String>,
}

/// The sentence a dialog shows under its fields for a failed write.
pub(super) fn failure_text(failure: &WriteFailure) -> (&'static str, Option<String>) {
    match failure {
        WriteFailure::Busy => (tr!("studio-write-blocked"), None),
        WriteFailure::Conflict => (tr!("studio-write-conflict"), None),
        WriteFailure::Other(detail) => (tr!("studio-write-failed"), Some(detail.clone())),
    }
}

impl StudioView {
    /// Why a write cannot start now, if it cannot: the game runs, or another
    /// write is still on its way.
    pub(super) fn write_blocked(&self) -> Option<&'static str> {
        if !self.can_write {
            Some(tr!("studio-write-blocked"))
        } else if self.writing {
            Some(tr!("studio-write-busy"))
        } else {
            None
        }
    }

    /// Makes the edited bytes with `edit` on the background executor from the
    /// chosen file as read, then asks the application to write them. The
    /// dialog behind `status` stays open and disabled until the answer.
    pub(super) fn submit(
        &mut self,
        status: Entity<Submission>,
        edit: impl FnOnce(&[u8]) -> Result<Edited, String> + Send + 'static,
        describe: Describe,
        cx: &mut Context<Self>,
    ) {
        self.submit_to(status, None, edit, describe, cx);
    }

    /// [`Self::submit`], writing a new file `beside` the chosen one instead
    /// when it is given; the chosen file stays as it was (T23).
    pub(super) fn submit_to(
        &mut self,
        status: Entity<Submission>,
        beside: Option<String>,
        edit: impl FnOnce(&[u8]) -> Result<Edited, String> + Send + 'static,
        describe: Describe,
        cx: &mut Context<Self>,
    ) {
        if let Some(why) = self.write_blocked() {
            status.update(cx, |status, cx| status.fail(why, None, cx));
            return;
        }
        let Some(opened) = &self.opened else {
            status.update(cx, |status, cx| {
                status.fail(tr!("studio-pick-first"), None, cx)
            });
            return;
        };
        let select = beside.clone();
        let (file, bytes, expected) = match beside {
            Some(beside) => (beside, opened.bytes.clone(), None),
            None => (
                opened.file.clone(),
                opened.bytes.clone(),
                Some(opened.info.clone()),
            ),
        };
        self.writing = true;
        status.update(cx, |status, cx| status.start(cx));
        let work = cx.background_executor().spawn(async move { edit(&bytes) });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(edited) => {
                        let (operation, done) = describe(edited.count);
                        this.pending = Some(Pending {
                            status: status.clone(),
                            done,
                            select,
                        });
                        this.send(Command::Apply {
                            file,
                            bytes: edited.bytes,
                            expected,
                            operation,
                        });
                    }
                    Err(detail) => {
                        this.writing = false;
                        status.update(cx, |status, cx| {
                            status.fail(tr!("studio-edit-failed"), Some(detail), cx)
                        });
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// The application's answer to [`Command::Apply`].
    pub(super) fn applied(
        &mut self,
        file: String,
        result: Result<SchematicLogEntry, WriteFailure>,
        cx: &mut Context<Self>,
    ) {
        self.writing = false;
        let pending = self.pending.take();
        let mut select = None;
        match (&result, pending) {
            (Ok(_), Some(pending)) => {
                pending.status.update(cx, |status, cx| status.finish(cx));
                self.toasts.push(Toast::success(pending.done));
                self.close_dialog = true;
                select = pending.select;
            }
            (Err(failure), Some(pending)) => {
                let (sentence, detail) = failure_text(failure);
                pending
                    .status
                    .update(cx, |status, cx| status.fail(sentence, detail, cx));
            }
            (Err(failure), None) => {
                let (sentence, detail) = failure_text(failure);
                let toast = Toast::error(sentence);
                self.toasts.push(match detail {
                    Some(detail) => toast.technical(detail),
                    None => toast,
                });
            }
            (Ok(_), None) => {}
        }
        if result.is_ok() {
            self.reread(&file, cx);
            self.send(Command::List);
        }
        if let Some(file) = select {
            self.open_file(file, cx);
        }
        self.want_history();
        self.send(Command::CanWrite);
    }

    /// Reads `file` again after it changed: its fingerprint and what it
    /// looked like are stale, and the inspector and viewport show it anew.
    pub(super) fn reread(&mut self, file: &str, cx: &mut Context<Self>) {
        self.tree.forget_fingerprint(file);
        self.infos.remove(file);
        if self.selected.as_deref() == Some(file) {
            self.selected = None;
            self.open_file(file.to_owned(), cx);
        }
    }

    /// Moves `files` into the backups: what deleting and "keep only this" do.
    fn remove(&mut self, files: Vec<String>, operation: SchematicOperation) {
        let files: Option<Vec<(String, FileInfo)>> = files
            .into_iter()
            .map(|file| self.infos.get(&file).cloned().map(|info| (file, info)))
            .collect();
        let Some(files) = files.filter(|files| !files.is_empty()) else {
            self.toasts
                .push(Toast::error(tr!("studio-remove-failed")).technical("file not read yet"));
            return;
        };
        self.writing = true;
        self.send(Command::Remove { files, operation });
    }

    /// The application's answer to [`Command::Remove`].
    pub(super) fn removed(&mut self, result: Result<Vec<SchematicLogEntry>, WriteFailure>) {
        self.writing = false;
        match result {
            Ok(entries) => {
                for entry in &entries {
                    self.tree.forget_fingerprint(&entry.file);
                    self.infos.remove(&entry.file);
                }
                let toast = match entries.as_slice() {
                    [entry] if entry.operation == DELETE => {
                        tr!("studio-deleted", file = tree::stem(&entry.file))
                    }
                    _ => tr!("studio-deduped", count = entries.len()),
                };
                self.toasts.push(Toast::success(toast));
            }
            Err(failure) => {
                let (_, detail) = failure_text(&failure);
                let toast = Toast::error(match failure {
                    WriteFailure::Busy => tr!("studio-write-blocked"),
                    WriteFailure::Conflict => tr!("studio-write-conflict"),
                    WriteFailure::Other(_) => tr!("studio-remove-failed"),
                });
                self.toasts.push(match detail {
                    Some(detail) => toast.technical(detail),
                    None => toast,
                });
            }
        }
        self.send(Command::List);
        self.want_history();
        self.send(Command::CanWrite);
    }

    /// 删除…: asks, then moves the chosen file into the backups.
    pub(super) fn ask_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(file) = self.selected.clone() else {
            return;
        };
        let studio = cx.entity().downgrade();
        let name = tree::stem(&file).to_owned();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (studio, file) = (studio.clone(), file.clone());
            alert
                .title(tr!("studio-delete-title", file = name.as_str()))
                .description(tr!("studio-delete-body"))
                .ok_text(tr!("common-delete"))
                .ok_variant(gpui_component::button::ButtonVariant::Danger)
                .cancel_text(tr!("common-cancel"))
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    let file = file.clone();
                    let _ = studio.update(cx, |view, cx| {
                        view.remove(
                            vec![file],
                            SchematicOperation {
                                kind: DELETE.into(),
                                summary: String::new(),
                            },
                        );
                        cx.notify();
                    });
                    true
                })
        });
    }

    /// 只保留这个: asks, then moves the group's other files into the backups.
    pub(super) fn ask_keep_only(
        &mut self,
        keep: String,
        others: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let studio = cx.entity().downgrade();
        let name = tree::stem(&keep).to_owned();
        let count = others.len();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (studio, others, name) = (studio.clone(), others.clone(), name.clone());
            alert
                .title(tr!("studio-keep-title", file = name.as_str()))
                .description(tr!("studio-keep-body", count = count))
                .ok_text(tr!("studio-keep-only"))
                .cancel_text(tr!("common-cancel"))
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    let (others, name) = (others.clone(), name.clone());
                    let _ = studio.update(cx, |view, cx| {
                        view.remove(
                            others,
                            SchematicOperation {
                                kind: DEDUPE.into(),
                                summary: tr!("studio-keep-summary", file = name.as_str()),
                            },
                        );
                        cx.notify();
                    });
                    true
                })
        });
    }

    /// The convert targets arrived; an open convert dialog lists them.
    pub(super) fn versions_arrived(
        &mut self,
        result: Result<Vec<(String, u32)>, String>,
        cx: &mut Context<Self>,
    ) {
        let loaded = match result {
            Ok(versions) => Loaded::Ready(versions),
            Err(detail) => Loaded::Failed(detail),
        };
        if let Some(OpenDialog::Convert(dialog)) = &self.dialog
            && let Some(dialog) = dialog.upgrade()
        {
            dialog.update(cx, |dialog, cx| dialog.set_versions(&loaded, cx));
        }
        self.versions = Some(loaded);
        self.want_compat(cx);
        self.want_replace_catalog();
    }

    /// Asks for the block table of the open file's own version when the
    /// replace dialog waits for it (T24): the version that writes its data
    /// version, else the nearest older one the bundle has.
    pub(super) fn want_replace_catalog(&mut self) {
        let (
            Some(OpenDialog::Replace(dialog)),
            Inspection::Ready(analysis),
            Some(Loaded::Ready(versions)),
        ) = (&self.dialog, &self.inspection, &self.versions)
        else {
            return;
        };
        if self.replace_catalog.is_some() || dialog.upgrade().is_none() {
            return;
        }
        // A file without a data version is drawn with the game version's
        // assets (T39), so its blocks are named by that version's table.
        let data_version = analysis
            .info
            .data_version
            .and_then(|version| u32::try_from(version).ok())
            .or_else(|| self.game_target().map(|(_, version)| version))
            .or_else(|| versions.iter().map(|(_, version)| *version).max());
        let Some(source) = data_version else {
            return;
        };
        let version = versions
            .iter()
            .filter(|(_, version)| *version <= source)
            .max_by_key(|(_, version)| *version)
            .or_else(|| versions.iter().min_by_key(|(_, version)| *version))
            .map(|(id, _)| id.clone());
        if let Some(version) = version {
            self.replace_catalog = Some(version.clone());
            self.want_catalogs(version, source);
        }
    }

    /// The saved tables arrived or were kept; an open replace dialog shows
    /// them.
    pub(super) fn schemes_arrived(
        &mut self,
        result: Result<lumilio_core::ReplaceSchemes, String>,
        cx: &mut Context<Self>,
    ) {
        let Ok(schemes) = result else {
            return;
        };
        self.schemes = schemes.clone();
        if let Some(OpenDialog::Replace(dialog)) = &self.dialog
            && let Some(dialog) = dialog.upgrade()
        {
            dialog.update(cx, |dialog, cx| dialog.set_schemes(schemes, cx));
        }
    }

    /// Keeps `schemes` as the saved tables and recent targets.
    pub(crate) fn save_schemes(&mut self, schemes: lumilio_core::ReplaceSchemes) {
        self.schemes = schemes.clone();
        self.send(Command::SaveSchemes(schemes));
    }

    /// Asks for the convert targets unless they are known or on their way.
    pub(super) fn want_versions(&mut self) {
        if matches!(self.versions, None | Some(Loaded::Failed(_))) {
            self.versions = Some(Loaded::Loading);
            self.send(Command::Versions);
        }
    }

    /// Asks for what a backward conversion to `target` is checked against.
    pub(super) fn want_catalogs(&self, target: String, source_data_version: u32) {
        self.send(Command::Catalogs {
            target,
            source_data_version,
        });
    }

    /// The catalogs arrived; an open convert dialog checks with them.
    pub(super) fn catalogs_arrived(
        &mut self,
        target: String,
        result: Result<(BlockCatalog, Option<BlockCatalog>), String>,
        cx: &mut Context<Self>,
    ) {
        self.compat_catalogs(&target, &result, cx);
        if self.replace_catalog.as_deref() == Some(target.as_str()) {
            self.replace_catalog = None;
            if let Some(OpenDialog::Replace(dialog)) = &self.dialog
                && let Some(dialog) = dialog.upgrade()
                && dialog.read(cx).wants_catalog()
            {
                let catalog = result.clone().map(|(catalog, _)| catalog);
                dialog.update(cx, |dialog, cx| dialog.set_catalog(catalog, cx));
            }
        }
        if let Some(OpenDialog::Convert(dialog)) = &self.dialog
            && let Some(dialog) = dialog.upgrade()
        {
            dialog.update(cx, |dialog, cx| {
                dialog.catalogs_arrived(&target, result, cx)
            });
        }
    }
}

/// A closure for a menu entry or key that acts on the studio.
pub(super) fn on_studio(
    studio: gpui::WeakEntity<StudioView>,
    act: impl Fn(&mut StudioView, &mut Window, &mut Context<StudioView>) + 'static,
) -> impl Fn(&mut Window, &mut App) + 'static {
    move |window, cx| {
        let _ = studio.update(cx, |view, cx| act(view, window, cx));
    }
}
