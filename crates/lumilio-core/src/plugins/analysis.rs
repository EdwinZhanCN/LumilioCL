use std::sync::Arc;

use lumilio_plugin_api::{AnalysisInput, Finding};

use super::{PluginHost, PluginStatus};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginFinding {
    /// Stable plugin id; rule ids remain local to this plugin.
    pub plugin: String,
    pub finding: Finding,
}

impl PluginHost {
    pub async fn analyze(&self, input: AnalysisInput) -> Vec<PluginFinding> {
        let input = Arc::new(input);
        let mut findings = Vec::new();
        for info in self.list().await {
            let input = input.clone();
            let result = self
                .call(&info.manifest.id, move |plugin, ctx| {
                    match plugin.analyzer() {
                        Some(analyzer) => analyzer.analyze(ctx, &input),
                        None => Ok(Vec::new()),
                    }
                })
                .await;
            if let Some(result) = result {
                findings.extend(result.into_iter().map(|finding| PluginFinding {
                    plugin: info.manifest.id.clone(),
                    finding,
                }));
            }
        }
        // A later analyzer can yield while preferences change. Recheck the
        // whole collection so disabled/failed plugins cannot keep a contribution.
        let enabled: Vec<_> = self
            .list()
            .await
            .into_iter()
            .filter(|info| info.status == PluginStatus::Enabled)
            .map(|info| info.manifest.id)
            .collect();
        findings.retain(|finding| enabled.contains(&finding.plugin));
        findings
    }
}
