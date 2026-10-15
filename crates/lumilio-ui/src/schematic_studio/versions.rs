//! Version facts (plan schematic-studio, T23): the open file's data version
//! by its game version's name, the instance's game version beside it, and
//! which of the file's blocks that game version does not have. The head
//! says both versions; when blocks are missing it carries an orange tag and
//! 转换到 y…, and 材料 marks those blocks.

use std::sync::Arc;

use gpui::Context;
use lumilio_core::BlockCatalog;
use lumilio_schematic::convert::{Catalogs, MISSING_BLOCK};

use super::dialogs::OpenDialog;
use super::{Inspection, Loaded, Placement, StudioView};
use crate::tr;

/// What the open file's blocks are against the game version.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) enum Compat {
    /// Not known: no file read yet, no game version or version list, or a
    /// game version the bundle does not know.
    #[default]
    Unknown,
    /// Waiting for the game version's block states.
    Checking { file: String, target: String },
    /// The blocks of `file` the game version `target` does not have, by
    /// the ids the file writes them with; empty when it has every one.
    Done {
        file: String,
        target: String,
        missing: Vec<String>,
    },
}

/// The name of data version `data_version` among `versions`: the game
/// version that writes it, or the number when the bundle knows none.
pub(crate) fn version_name(versions: &[(String, u32)], data_version: i32) -> String {
    versions
        .iter()
        .find(|(_, version)| i64::from(*version) == i64::from(data_version))
        .map_or_else(
            || tr!("studio-convert-data-version", version = data_version),
            |(id, _)| id.clone(),
        )
}

impl StudioView {
    /// The instance's game version, once known.
    pub(crate) fn game_version(&self) -> Option<&str> {
        match &self.game {
            Some(Loaded::Ready(version)) => Some(version),
            _ => None,
        }
    }

    /// The game version and its data version, when the bundle knows it.
    pub(super) fn game_target(&self) -> Option<(String, u32)> {
        let game = self.game_version()?;
        let Some(Loaded::Ready(versions)) = &self.versions else {
            return None;
        };
        versions.iter().find(|(id, _)| id == game).cloned()
    }

    /// `x 投影 · 游戏 y`, or one version when the two are the same; just the
    /// file's when the game version is not known.
    pub(super) fn version_words(&self) -> Option<String> {
        let (Inspection::Ready(analysis), Some(Loaded::Ready(versions))) =
            (&self.inspection, &self.versions)
        else {
            return None;
        };
        let file = version_name(versions, analysis.info.data_version);
        Some(match self.game_version() {
            Some(game) if game == file => tr!("studio-caption-version", version = file.as_str()),
            Some(game) => tr!(
                "studio-caption-versions",
                schematic = file.as_str(),
                game = game
            ),
            None => tr!("studio-caption-schematic", version = file.as_str()),
        })
    }

    /// The game version arrived while the convert dialog is open: it
    /// becomes the dialog's target when nothing is chosen there yet.
    pub(super) fn game_to_dialog(&self, cx: &mut Context<Self>) {
        let (Some(game), Some(OpenDialog::Convert(dialog))) = (self.game_version(), &self.dialog)
        else {
            return;
        };
        let Some(dialog) = dialog.upgrade() else {
            return;
        };
        let versions = self.versions.as_ref().unwrap_or(&Loaded::Loading);
        dialog.update(cx, |dialog, cx| {
            dialog.prefer(game);
            dialog.set_versions(versions, cx);
        });
    }

    /// The blocks the game version lacks, when that is known for the open
    /// file; empty when it lacks none.
    pub(crate) fn missing_blocks(&self) -> &[String] {
        match (&self.compat, &self.selected) {
            (Compat::Done { file, missing, .. }, Some(selected)) if file == selected => missing,
            _ => &[],
        }
    }

    /// The game version a conversion would bring the open file to, when
    /// that version lacks some of its blocks.
    pub(crate) fn compat_target(&self) -> Option<&str> {
        match &self.compat {
            Compat::Done {
                file,
                target,
                missing,
            } if self.selected.as_ref() == Some(file) && !missing.is_empty() => Some(target),
            _ => None,
        }
    }

    /// Checks the open file against the game version once both are known:
    /// a file no newer than the game converts forward without losing a
    /// block; a newer one is converted on the background executor and
    /// checked against the game version's block states.
    pub(super) fn want_compat(&mut self, cx: &mut Context<Self>) {
        if self.placement != Placement::Window {
            return;
        }
        self.want_versions();
        if self.game.is_none() {
            self.game = Some(Loaded::Loading);
            self.send(super::Command::Game);
        }
        let (Some(file), Inspection::Ready(analysis)) = (&self.selected, &self.inspection) else {
            return;
        };
        let Some((target, data_version)) = self.game_target() else {
            return;
        };
        let current = (&self.compat, file, &target);
        match current {
            (
                Compat::Checking { file: f, target: t }
                | Compat::Done {
                    file: f, target: t, ..
                },
                file,
                target,
            ) if f == file && t == target => {
                return;
            }
            _ => {}
        }
        let source = analysis.info.data_version;
        if i64::from(source) <= i64::from(data_version) {
            self.compat = Compat::Done {
                file: file.clone(),
                target,
                missing: Vec::new(),
            };
            cx.notify();
            return;
        }
        let Ok(source) = u32::try_from(source) else {
            return;
        };
        self.compat = Compat::Checking {
            file: file.clone(),
            target: target.clone(),
        };
        self.want_catalogs(target, source);
    }

    /// The game version's block states arrived for the check (or could not
    /// be had, which leaves it unknown).
    pub(super) fn compat_catalogs(
        &mut self,
        target: &str,
        result: &Result<(BlockCatalog, Option<BlockCatalog>), String>,
        cx: &mut Context<Self>,
    ) {
        let Compat::Checking {
            file,
            target: waiting,
        } = &self.compat
        else {
            return;
        };
        if waiting != target {
            return;
        }
        let (file, target) = (file.clone(), waiting.clone());
        let Ok((catalog, source)) = result else {
            self.compat = Compat::Unknown;
            return;
        };
        let (Some(opened), Some((_, data_version))) = (&self.opened, self.game_target()) else {
            return;
        };
        if opened.file != file {
            return;
        }
        let bytes: Arc<Vec<u8>> = opened.bytes.clone();
        let catalogs = Catalogs {
            target: catalog.clone(),
            source: source.clone(),
        };
        let work = cx.background_executor().spawn(async move {
            let mut schematic = lumilio_schematic::load_litematic(&bytes).ok()?;
            let target = i32::try_from(data_version).ok()?;
            let conversion =
                lumilio_schematic::convert::convert(&mut schematic, target, Some(&catalogs));
            let mut missing: Vec<String> = conversion
                .losses
                .into_iter()
                .filter(|loss| loss.kind == MISSING_BLOCK && loss.path.starts_with("minecraft:"))
                .map(|loss| loss.path)
                .collect();
            missing.sort();
            missing.dedup();
            Some(missing)
        });
        cx.spawn(async move |this, cx| {
            let missing = work.await;
            let _ = this.update(cx, |this, cx| {
                if this.compat
                    != (Compat::Checking {
                        file: file.clone(),
                        target: target.clone(),
                    })
                {
                    return;
                }
                this.compat = match missing {
                    Some(missing) => Compat::Done {
                        file,
                        target,
                        missing,
                    },
                    None => Compat::Unknown,
                };
                cx.notify();
            });
        })
        .detach();
    }
}
