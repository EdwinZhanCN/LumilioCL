//! The launch progress contract shared by launch services and the UI.
//!
//! A launch walks four phases in a fixed order. Whatever produces the work
//! (verification, installation, process supervision) reports it as
//! [`LaunchSignal`]s; [`LaunchSession`] reduces them into a status and one
//! overall progress value with two guarantees the UI relies on:
//!
//! - phases only move forward, and
//! - overall progress never decreases.
//!
//! The session carries no display text; the UI maps it to copy.

use crate::install::{InstallEvent, InstallStage};

/// Overall progress is tracked in basis points so the session stays `Eq`.
const FULL: u32 = 10_000;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LaunchPhase {
    /// Checking the files that are already on disk.
    Verifying,
    /// Fetching the client and its libraries.
    Libraries,
    /// Fetching and laying out game assets.
    Assets,
    /// Publishing natives and the manifest, then starting the game process.
    Starting,
}

impl LaunchPhase {
    pub const ORDER: [Self; 4] = [
        Self::Verifying,
        Self::Libraries,
        Self::Assets,
        Self::Starting,
    ];

    pub const fn index(self) -> usize {
        match self {
            Self::Verifying => 0,
            Self::Libraries => 1,
            Self::Assets => 2,
            Self::Starting => 3,
        }
    }

    /// Share of overall progress, in basis points. Downloads dominate.
    const fn weight(self) -> u32 {
        match self {
            Self::Verifying => 1_000,
            Self::Libraries => 3_500,
            Self::Assets => 4_000,
            Self::Starting => 1_500,
        }
    }

    /// Overall progress at which this phase begins, in basis points.
    const fn start(self) -> u32 {
        match self {
            Self::Verifying => 0,
            Self::Libraries => 1_000,
            Self::Assets => 4_500,
            Self::Starting => 8_500,
        }
    }

    /// The phase an installation stage belongs to.
    pub const fn of_install_stage(stage: InstallStage) -> Self {
        match stage {
            InstallStage::InitialTransfers => Self::Libraries,
            InstallStage::AssetCatalogDecode
            | InstallStage::AssetTransfers
            | InstallStage::AssetViews => Self::Assets,
            InstallStage::NativePublication | InstallStage::ManifestPublication => Self::Starting,
        }
    }
}

/// Why a launch did not reach a running game.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LaunchFailure {
    /// A preparation step failed; the message is diagnostic detail.
    Step { message: String },
    /// The game process ended before it finished starting.
    ExitedEarly { code: Option<i32> },
}

/// Input to a [`LaunchSession`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LaunchSignal {
    /// Work for this phase has begun.
    Phase(LaunchPhase),
    /// Item progress within the current phase.
    Progress {
        done: u64,
        total: u64,
    },
    /// The game is up and running.
    Running,
    /// The game process ended.
    Exited {
        code: Option<i32>,
    },
    Failed(LaunchFailure),
    Cancelled,
}

impl LaunchSignal {
    /// Translates an installer event into a launch signal, if it carries one.
    ///
    /// Only stages that close a phase report completion; completing a
    /// sub-stage (for example decoding the asset catalog) must not claim the
    /// whole phase is done.
    pub fn from_install(event: &InstallEvent) -> Option<Self> {
        match event {
            InstallEvent::StageStarted(stage) => {
                Some(Self::Phase(LaunchPhase::of_install_stage(*stage)))
            }
            InstallEvent::StageCompleted {
                stage: InstallStage::InitialTransfers | InstallStage::AssetViews,
                items,
            } => {
                let items = (*items as u64).max(1);
                Some(Self::Progress {
                    done: items,
                    total: items,
                })
            }
            InstallEvent::StageCompleted { .. } | InstallEvent::Completed { .. } => None,
            InstallEvent::StageFailed { message, .. } => Some(Self::Failed(LaunchFailure::Step {
                message: message.clone(),
            })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LaunchStatus {
    Preparing(LaunchPhase),
    Running,
    Exited {
        code: Option<i32>,
    },
    Failed {
        phase: LaunchPhase,
        failure: LaunchFailure,
    },
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchSession {
    status: LaunchStatus,
    items: Option<(u64, u64)>,
    reached: u32,
}

impl Default for LaunchSession {
    fn default() -> Self {
        Self::new()
    }
}

impl LaunchSession {
    pub const fn new() -> Self {
        Self {
            status: LaunchStatus::Preparing(LaunchPhase::Verifying),
            items: None,
            reached: 0,
        }
    }

    pub fn status(&self) -> &LaunchStatus {
        &self.status
    }

    /// The phase in progress, or the phase a failure happened in.
    pub fn phase(&self) -> Option<LaunchPhase> {
        match self.status {
            LaunchStatus::Preparing(phase) | LaunchStatus::Failed { phase, .. } => Some(phase),
            _ => None,
        }
    }

    /// Item counts reported for the current phase, as (done, total).
    pub fn items(&self) -> Option<(u64, u64)> {
        self.items
    }

    /// Overall progress in `[0, 1]`; never decreases over the session.
    pub fn fraction(&self) -> f32 {
        self.reached as f32 / FULL as f32
    }

    /// True once no further signal can change the session.
    pub fn is_finished(&self) -> bool {
        matches!(
            self.status,
            LaunchStatus::Exited { .. } | LaunchStatus::Failed { .. } | LaunchStatus::Cancelled
        )
    }

    /// Applies a signal. Returns whether the session changed.
    pub fn apply(&mut self, signal: LaunchSignal) -> bool {
        let before = self.clone();
        match (&self.status, signal) {
            (
                LaunchStatus::Exited { .. } | LaunchStatus::Failed { .. } | LaunchStatus::Cancelled,
                _,
            ) => {}
            (LaunchStatus::Running, LaunchSignal::Exited { code }) => {
                self.status = LaunchStatus::Exited { code };
            }
            (LaunchStatus::Running, _) => {}
            (&LaunchStatus::Preparing(current), signal) => match signal {
                LaunchSignal::Phase(next) if next > current => {
                    self.status = LaunchStatus::Preparing(next);
                    self.items = None;
                    self.reach(next.start());
                }
                LaunchSignal::Phase(_) => {}
                LaunchSignal::Progress { done, total } if total > 0 => {
                    let done = done.min(total);
                    self.items = Some((done, total));
                    let within = (current.weight() as u64 * done / total) as u32;
                    self.reach(current.start() + within);
                }
                LaunchSignal::Progress { .. } => {}
                LaunchSignal::Running => {
                    self.status = LaunchStatus::Running;
                    self.items = None;
                    self.reach(FULL);
                }
                LaunchSignal::Exited { code } => {
                    self.status = LaunchStatus::Failed {
                        phase: current,
                        failure: LaunchFailure::ExitedEarly { code },
                    };
                }
                LaunchSignal::Failed(failure) => {
                    self.status = LaunchStatus::Failed {
                        phase: current,
                        failure,
                    };
                }
                LaunchSignal::Cancelled => self.status = LaunchStatus::Cancelled,
            },
        }
        *self != before
    }

    fn reach(&mut self, value: u32) {
        self.reached = self.reached.max(value.min(FULL));
    }
}

#[cfg(test)]
mod tests;
