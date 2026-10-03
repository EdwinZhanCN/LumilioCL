/// A prerequisite that must be available before Minecraft can launch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchRequirement {
    Account,
    GameVersion,
    JavaRuntime,
}

impl LaunchRequirement {
    const ALL: [Self; 3] = [Self::Account, Self::GameVersion, Self::JavaRuntime];
}

/// Snapshot of the prerequisites used to decide whether a launch can start.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LaunchReadiness {
    account_available: bool,
    game_version_installed: bool,
    java_runtime_available: bool,
}

impl LaunchReadiness {
    #[must_use]
    pub const fn new(
        account_available: bool,
        game_version_installed: bool,
        java_runtime_available: bool,
    ) -> Self {
        Self {
            account_available,
            game_version_installed,
            java_runtime_available,
        }
    }

    /// Returns the unsatisfied launch prerequisites in a stable display order.
    pub fn missing_requirements(&self) -> impl Iterator<Item = LaunchRequirement> + '_ {
        LaunchRequirement::ALL
            .into_iter()
            .filter(|requirement| !self.satisfies(*requirement))
    }

    #[must_use]
    pub const fn is_ready(&self) -> bool {
        self.account_available && self.game_version_installed && self.java_runtime_available
    }

    const fn satisfies(&self, requirement: LaunchRequirement) -> bool {
        match requirement {
            LaunchRequirement::Account => self.account_available,
            LaunchRequirement::GameVersion => self.game_version_installed,
            LaunchRequirement::JavaRuntime => self.java_runtime_available,
        }
    }
}

#[cfg(test)]
mod tests {
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
}
