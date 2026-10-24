/// The source a project reference without one means: everything recorded
/// before there was more than one source came from Modrinth.
pub const DEFAULT_SOURCE: &str = "lumilio.modrinth";

/// A project in one content source, such as the pack an instance was made
/// from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectOrigin {
    pub source: String,
    pub project: String,
}

impl ProjectOrigin {
    /// As stored: the bare ID for [`DEFAULT_SOURCE`] (as before there were
    /// several sources), `<source>/<id>` otherwise. Plugin and project IDs
    /// never contain `/`.
    #[must_use]
    pub fn stored(&self) -> String {
        if self.source == DEFAULT_SOURCE {
            self.project.clone()
        } else {
            format!("{}/{}", self.source, self.project)
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Self {
        match text.split_once('/') {
            Some((source, project)) => Self {
                source: source.to_owned(),
                project: project.to_owned(),
            },
            None => Self {
                source: DEFAULT_SOURCE.to_owned(),
                project: text.to_owned(),
            },
        }
    }
}
