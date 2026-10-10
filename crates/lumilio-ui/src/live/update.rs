/// The update status exposed to Settings and the navigation key. Failure
/// details stay technical; the UI owns the user-facing wording.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum UpdateStatus {
    #[default]
    NotChecked,
    Checking,
    Downloading {
        version: String,
    },
    Ready {
        version: String,
    },
    UpToDate,
    Unavailable(UpdateUnavailability),
    Failed {
        detail: String,
    },
}

impl UpdateStatus {
    pub fn ready_version(&self) -> Option<&str> {
        match self {
            Self::Ready { version } => Some(version),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateUnavailability {
    DebugBuild,
    PortableWindows,
    PackageManagedLinux,
    UnsupportedInstall,
}
