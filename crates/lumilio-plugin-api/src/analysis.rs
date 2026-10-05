use serde::{Deserialize, Serialize};

use crate::{HostContext, PluginError};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct GameFacts {
    pub game_version: String,
    /// Stable loader id, such as "vanilla", "fabric", or "neoforge".
    pub loader: String,
    pub loader_version: Option<String>,
    /// Effective Java major if the host can determine it.
    pub java_major: Option<u32>,
    pub mods: Vec<ModFact>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ModFact {
    pub id: String,
    pub version: Option<String>,
    pub file: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AnalysisSource {
    LatestLog,
    CrashReport,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AnalysisInput {
    pub text: String,
    pub source: AnalysisSource,
    pub game: GameFacts,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Finding {
    /// Stable rule id local to the plugin; the host does not interpret it.
    pub rule: String,
    pub severity: Severity,
    pub title: String,
    pub advice: String,
    pub evidence: Option<String>,
}

pub trait Analyzer: Send + Sync {
    fn analyze(
        &self,
        ctx: &dyn HostContext,
        input: &AnalysisInput,
    ) -> Result<Vec<Finding>, PluginError>;
}
