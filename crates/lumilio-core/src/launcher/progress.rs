use super::output::OutputFile;
use super::types::LaunchUpdate;
use crate::install::{AssetIndex, InstallEvent, InstallStage, InstallationPlan};
use crate::instance::Loader;
use crate::launch_session::{LaunchFailure, LaunchPhase, LaunchSignal};
use crate::process::GameEvent;
use std::collections::VecDeque;
use std::path::PathBuf;
use tokio::sync::mpsc;

/// Loaders installed by running their official installer's processors.
pub(super) const fn uses_installer(loader: Loader) -> bool {
    matches!(loader, Loader::Forge | Loader::NeoForge)
}

/// Counts finished transfers inside the current download stage.
#[derive(Default)]
pub(super) struct ItemProgress {
    pub(super) active: bool,
    pub(super) done: u64,
    pub(super) total: u64,
}

impl ItemProgress {
    pub(super) fn begin(&mut self, total: u64, updates: &mpsc::UnboundedSender<LaunchUpdate>) {
        self.active = total > 0;
        self.done = 0;
        self.total = total;
        if self.active {
            let _ = updates.send(LaunchUpdate::Signal(LaunchSignal::Progress {
                done: 0,
                total,
            }));
        }
    }

    pub(super) fn completed(&mut self, updates: &mpsc::UnboundedSender<LaunchUpdate>) {
        if !self.active {
            return;
        }
        self.done = (self.done + 1).min(self.total);
        let _ = updates.send(LaunchUpdate::Signal(LaunchSignal::Progress {
            done: self.done,
            total: self.total,
        }));
    }
}

pub(super) fn handle_stage(
    event: &InstallEvent,
    plan: &InstallationPlan,
    progress: &mut ItemProgress,
    updates: &mpsc::UnboundedSender<LaunchUpdate>,
) {
    match event {
        InstallEvent::StageStarted(InstallStage::InitialTransfers) => {
            let _ = updates.send(LaunchUpdate::Signal(LaunchSignal::Phase(
                LaunchPhase::Libraries,
            )));
            progress.begin(plan.initial_artifacts().len() as u64, updates);
            return;
        }
        InstallEvent::StageStarted(InstallStage::AssetTransfers) => {
            let _ = updates.send(LaunchUpdate::Signal(LaunchSignal::Phase(
                LaunchPhase::Assets,
            )));
            progress.begin(asset_object_count(plan), updates);
            return;
        }
        InstallEvent::StageStarted(_) => progress.active = false,
        _ => {}
    }
    if let Some(signal) = LaunchSignal::from_install(event) {
        let _ = updates.send(LaunchUpdate::Signal(signal));
    }
}

/// Unique asset objects listed by the already-downloaded asset catalog.
pub(super) fn asset_object_count(plan: &InstallationPlan) -> u64 {
    plan.asset_catalog_artifact()
        .and_then(|artifact| std::fs::read_to_string(artifact.request().destination()).ok())
        .and_then(|json| AssetIndex::decode_json(&json).ok())
        .map_or(0, |index| index.unique_objects().len() as u64)
}

/// How many of its last lines a game that quits while starting shows.
const EARLY_OUTPUT_LINES: usize = 50;

/// Passes the game's events on as launch updates, and into the launch's
/// output file when there is one. Until the game is up it keeps the last
/// lines it printed, which an early exit carries to technical details: a
/// game that quits that soon may not have written a log.
#[derive(Default)]
pub(super) struct GameForwarder {
    running: bool,
    early_output: VecDeque<String>,
    file: Option<OutputFile>,
}

impl GameForwarder {
    pub(super) fn recording(file: Option<OutputFile>) -> Self {
        Self {
            file,
            ..Self::default()
        }
    }

    pub(super) fn forward(
        &mut self,
        event: &GameEvent,
        updates: &mpsc::UnboundedSender<LaunchUpdate>,
    ) {
        match event {
            GameEvent::Running => {
                self.running = true;
                self.early_output.clear();
            }
            GameEvent::Line { stream, text } => {
                if !self.running {
                    if self.early_output.len() == EARLY_OUTPUT_LINES {
                        self.early_output.pop_front();
                    }
                    self.early_output.push_back(text.clone());
                }
                if let Some(file) = &mut self.file {
                    file.line(text);
                }
                let _ = updates.send(LaunchUpdate::Log {
                    stream: *stream,
                    text: text.clone(),
                });
            }
            GameEvent::Exited {
                code,
                ran_for,
                killed,
            } => {
                if let Some(file) = &mut self.file {
                    file.ended(*code, *ran_for, *killed);
                }
            }
            GameEvent::Spawned { .. } => {}
        }
        if let Some(mut signal) = event.signal(self.running) {
            if let LaunchSignal::Failed(LaunchFailure::ExitedEarly { output, .. }) = &mut signal {
                *output = std::mem::take(&mut self.early_output).into();
            }
            let _ = updates.send(LaunchUpdate::Signal(signal));
        }
    }
}

/// Helper for callers that only need the path a runtime list should start
/// from; kept here so the app and tests share one definition of the layout.
#[must_use]
pub fn runtimes_root(launcher_root: &std::path::Path) -> PathBuf {
    crate::layout::Layout::new(launcher_root).runtimes()
}
