use super::{LaunchReadiness, LaunchRequirement};

#[test]
fn an_unconfigured_launcher_reports_every_missing_requirement() {
    let readiness = LaunchReadiness::default();

    assert_eq!(
        readiness.missing_requirements().collect::<Vec<_>>(),
        vec![
            LaunchRequirement::Account,
            LaunchRequirement::GameVersion,
            LaunchRequirement::JavaRuntime,
        ]
    );
    assert!(!readiness.is_ready());
}

#[test]
fn a_launcher_with_every_requirement_is_ready() {
    let readiness = LaunchReadiness::new(true, true, true);

    assert!(readiness.missing_requirements().next().is_none());
    assert!(readiness.is_ready());
}

#[test]
fn a_partial_configuration_reports_only_its_remaining_work() {
    let readiness = LaunchReadiness::new(true, false, true);

    assert_eq!(
        readiness.missing_requirements().collect::<Vec<_>>(),
        vec![LaunchRequirement::GameVersion]
    );
}
