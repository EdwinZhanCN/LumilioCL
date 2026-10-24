use lumilio_plugin_api::{
    API_VERSION, AnalysisInput, Analyzer, Finding, HostContext, Manifest, Plugin, PluginError,
};

use super::{rules, text};

pub const ID: &str = "lumilio.crash-analyzer";

pub struct CrashAnalyzer;

impl Plugin for CrashAnalyzer {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: ID.into(),
            name: text::name(),
            description: text::description(),
            version: env!("CARGO_PKG_VERSION").into(),
            api: API_VERSION,
            default_enabled: true,
            permissions: Vec::new(),
            settings: Vec::new(),
        }
    }

    fn analyzer(&self) -> Option<&dyn Analyzer> {
        Some(self)
    }
}

impl Analyzer for CrashAnalyzer {
    fn analyze(
        &self,
        ctx: &dyn HostContext,
        input: &AnalysisInput,
    ) -> Result<Vec<Finding>, PluginError> {
        Ok(rules::analyze(&input.text, ctx.locale()))
    }
}
