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
mod tests {
    use std::path::PathBuf;

    use super::{FULL, LaunchFailure, LaunchPhase, LaunchSession, LaunchSignal, LaunchStatus};
    use crate::install::{InstallEvent, InstallStage};

    fn progress(done: u64, total: u64) -> LaunchSignal {
        LaunchSignal::Progress { done, total }
    }

    #[test]
    fn phase_weights_partition_the_whole_bar() {
        let mut start = 0;
        for phase in LaunchPhase::ORDER {
            assert_eq!(phase.start(), start);
            start += phase.weight();
        }
        assert_eq!(start, FULL);
    }

    #[test]
    fn progress_is_monotonic_even_with_noisy_input() {
        let mut session = LaunchSession::new();
        let noisy = [
            progress(3, 10),
            progress(1, 10), // regressing count
            LaunchSignal::Phase(LaunchPhase::Libraries),
            progress(50, 100),
            LaunchSignal::Phase(LaunchPhase::Verifying), // backwards phase
            progress(10, 400),                           // new, larger total
            progress(999, 100),                          // overshoot
            LaunchSignal::Phase(LaunchPhase::Assets),
            progress(0, 0), // empty total
            LaunchSignal::Phase(LaunchPhase::Starting),
            LaunchSignal::Running,
        ];
        let mut last = session.fraction();
        for signal in noisy {
            session.apply(signal);
            assert!(session.fraction() >= last, "{session:?}");
            last = session.fraction();
        }
        assert_eq!(session.fraction(), 1.);
        assert_eq!(session.status(), &LaunchStatus::Running);
    }

    #[test]
    fn phases_only_move_forward_and_reset_item_counts() {
        let mut session = LaunchSession::new();
        assert!(session.apply(LaunchSignal::Phase(LaunchPhase::Assets)));
        assert!(!session.apply(LaunchSignal::Phase(LaunchPhase::Libraries)));
        assert_eq!(session.phase(), Some(LaunchPhase::Assets));
        session.apply(progress(5, 10));
        assert_eq!(session.items(), Some((5, 10)));
        session.apply(LaunchSignal::Phase(LaunchPhase::Starting));
        assert_eq!(session.items(), None);
        assert!((session.fraction() - 0.85).abs() < 1e-6);
    }

    #[test]
    fn an_exit_before_running_is_a_failure_in_the_current_phase() {
        let mut session = LaunchSession::new();
        session.apply(LaunchSignal::Phase(LaunchPhase::Starting));
        session.apply(LaunchSignal::Exited { code: Some(1) });
        assert_eq!(
            session.status(),
            &LaunchStatus::Failed {
                phase: LaunchPhase::Starting,
                failure: LaunchFailure::ExitedEarly { code: Some(1) },
            }
        );
        assert!(session.is_finished());
        assert!(!session.apply(LaunchSignal::Running), "terminal is final");
    }

    #[test]
    fn a_running_game_only_listens_for_its_exit() {
        let mut session = LaunchSession::new();
        session.apply(LaunchSignal::Running);
        assert!(!session.apply(LaunchSignal::Cancelled));
        assert!(!session.apply(LaunchSignal::Phase(LaunchPhase::Assets)));
        assert!(session.apply(LaunchSignal::Exited { code: Some(0) }));
        assert_eq!(session.status(), &LaunchStatus::Exited { code: Some(0) });
        assert_eq!(session.phase(), None);
    }

    #[test]
    fn cancelling_ends_preparation() {
        let mut session = LaunchSession::new();
        session.apply(LaunchSignal::Phase(LaunchPhase::Libraries));
        assert!(session.apply(LaunchSignal::Cancelled));
        assert_eq!(session.status(), &LaunchStatus::Cancelled);
        assert!(!session.apply(progress(1, 1)));
    }

    #[test]
    fn install_events_map_onto_phases_without_claiming_early_completion() {
        let started = |stage| LaunchSignal::from_install(&InstallEvent::StageStarted(stage));
        assert_eq!(
            started(InstallStage::InitialTransfers),
            Some(LaunchSignal::Phase(LaunchPhase::Libraries))
        );
        assert_eq!(
            started(InstallStage::AssetTransfers),
            Some(LaunchSignal::Phase(LaunchPhase::Assets))
        );
        assert_eq!(
            started(InstallStage::ManifestPublication),
            Some(LaunchSignal::Phase(LaunchPhase::Starting))
        );

        let completed = |stage, items| {
            LaunchSignal::from_install(&InstallEvent::StageCompleted { stage, items })
        };
        assert_eq!(
            completed(InstallStage::AssetCatalogDecode, 1),
            None,
            "a sub-stage does not finish the asset phase"
        );
        assert_eq!(completed(InstallStage::AssetViews, 0), Some(progress(1, 1)));
        assert_eq!(
            LaunchSignal::from_install(&InstallEvent::StageFailed {
                stage: InstallStage::AssetTransfers,
                message: "checksum mismatch".into(),
            }),
            Some(LaunchSignal::Failed(LaunchFailure::Step {
                message: "checksum mismatch".into()
            }))
        );
        assert_eq!(
            LaunchSignal::from_install(&InstallEvent::Completed {
                manifest: PathBuf::from("m.json")
            }),
            None
        );
    }
}
