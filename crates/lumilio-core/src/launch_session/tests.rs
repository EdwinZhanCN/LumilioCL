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

    let completed =
        |stage, items| LaunchSignal::from_install(&InstallEvent::StageCompleted { stage, items });
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
