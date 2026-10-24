//! Log analysis contributes data only. The host owns files and execution.

mod rules;
#[cfg(test)]
mod tests;
mod text;

pub use crash_analyzer::{CrashAnalyzer, ID};
#[cfg(test)]
use lumilio_plugin_api::{AnalysisInput, Analyzer, Finding, HostContext, Plugin, PluginError};

mod crash_analyzer;
