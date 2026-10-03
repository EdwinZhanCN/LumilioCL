use std::collections::BTreeMap;

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlatformFamily {
    Windows,
    MacOs,
    Linux,
    Bsd,
    Other(String),
}

impl PlatformFamily {
    #[must_use]
    pub fn protocol_name(&self) -> &str {
        match self {
            Self::Windows => "windows",
            Self::MacOs => "osx",
            Self::Linux | Self::Bsd => "linux",
            Self::Other(name) => name,
        }
    }

    fn matches_name(&self, name: &str) -> bool {
        match name.to_ascii_lowercase().as_str() {
            "windows" | "win" => matches!(self, Self::Windows),
            "osx" | "macos" | "mac" => matches!(self, Self::MacOs),
            "linux" => matches!(self, Self::Linux | Self::Bsd),
            "bsd" => matches!(self, Self::Bsd),
            other => matches!(self, Self::Other(current) if current.eq_ignore_ascii_case(other)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachineArchitecture {
    X86,
    X86_64,
    Arm32,
    Arm64,
    Other(String),
}

impl MachineArchitecture {
    #[must_use]
    pub const fn bit_width(&self) -> u8 {
        match self {
            Self::X86 | Self::Arm32 => 32,
            Self::X86_64 | Self::Arm64 => 64,
            Self::Other(_) => 0,
        }
    }

    #[must_use]
    pub fn protocol_name(&self) -> &str {
        match self {
            Self::X86 => "x86",
            Self::X86_64 => "x86_64",
            Self::Arm32 => "arm32",
            Self::Arm64 => "arm64",
            Self::Other(name) => name,
        }
    }

    fn matches_name(&self, name: &str) -> bool {
        match name.to_ascii_lowercase().as_str() {
            "x86" | "i386" | "i486" | "i586" | "i686" | "32" => {
                matches!(self, Self::X86)
            }
            "x86_64" | "amd64" | "x64" | "64" => matches!(self, Self::X86_64),
            "arm" | "arm32" | "armv7" | "armv7l" => matches!(self, Self::Arm32),
            "arm64" | "aarch64" => matches!(self, Self::Arm64),
            other => matches!(self, Self::Other(current) if current.eq_ignore_ascii_case(other)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostProfile {
    platform: PlatformFamily,
    architecture: MachineArchitecture,
    platform_version: String,
    features: BTreeMap<String, bool>,
}

impl HostProfile {
    #[must_use]
    pub fn new(
        platform: PlatformFamily,
        architecture: MachineArchitecture,
        platform_version: impl Into<String>,
    ) -> Self {
        Self {
            platform,
            architecture,
            platform_version: platform_version.into(),
            features: BTreeMap::new(),
        }
    }

    /// The machine this process runs on.
    ///
    /// The platform version is left empty: the only version-gated rules in
    /// game metadata target old Windows releases, and an empty version simply
    /// fails to match them.
    #[must_use]
    pub fn current() -> Self {
        let platform = match std::env::consts::OS {
            "windows" => PlatformFamily::Windows,
            "macos" => PlatformFamily::MacOs,
            "linux" => PlatformFamily::Linux,
            "freebsd" | "openbsd" | "netbsd" | "dragonfly" => PlatformFamily::Bsd,
            other => PlatformFamily::Other(other.to_owned()),
        };
        let architecture = match std::env::consts::ARCH {
            "x86" => MachineArchitecture::X86,
            "x86_64" => MachineArchitecture::X86_64,
            "arm" => MachineArchitecture::Arm32,
            "aarch64" => MachineArchitecture::Arm64,
            other => MachineArchitecture::Other(other.to_owned()),
        };
        Self::new(platform, architecture, "")
    }

    #[must_use]
    pub fn with_feature(mut self, name: impl Into<String>, enabled: bool) -> Self {
        self.features.insert(name.into(), enabled);
        self
    }

    #[must_use]
    pub fn platform(&self) -> &PlatformFamily {
        &self.platform
    }

    #[must_use]
    pub fn architecture(&self) -> &MachineArchitecture {
        &self.architecture
    }

    #[must_use]
    pub fn platform_version(&self) -> &str {
        &self.platform_version
    }

    #[must_use]
    pub fn features(&self) -> &BTreeMap<String, bool> {
        &self.features
    }

    #[must_use]
    pub fn allows(&self, rules: &[CompatibilityRule]) -> bool {
        if rules.is_empty() {
            return true;
        }

        let mut result = RuleDecision::Block;
        for rule in rules {
            if rule.matches(self) {
                result = rule.decision;
            }
        }
        result == RuleDecision::Permit
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuleDecision {
    #[serde(rename = "allow")]
    Permit,
    #[serde(rename = "disallow")]
    Block,
}

fn permit() -> RuleDecision {
    RuleDecision::Permit
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CompatibilityRule {
    #[serde(rename = "action", default = "permit")]
    decision: RuleDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    os: Option<PlatformRequirement>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    features: BTreeMap<String, bool>,
}

impl CompatibilityRule {
    #[must_use]
    pub fn new(decision: RuleDecision) -> Self {
        Self {
            decision,
            os: None,
            features: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn requiring_platform(mut self, name: impl Into<String>) -> Self {
        self.os.get_or_insert_with(Default::default).name = Some(name.into());
        self
    }

    #[must_use]
    pub fn requiring_architecture(mut self, pattern: impl Into<String>) -> Self {
        self.os.get_or_insert_with(Default::default).arch = Some(pattern.into());
        self
    }

    #[must_use]
    pub fn requiring_platform_version(mut self, pattern: impl Into<String>) -> Self {
        self.os.get_or_insert_with(Default::default).version = Some(pattern.into());
        self
    }

    #[must_use]
    pub fn requiring_feature(mut self, name: impl Into<String>, enabled: bool) -> Self {
        self.features.insert(name.into(), enabled);
        self
    }

    #[must_use]
    pub const fn decision(&self) -> RuleDecision {
        self.decision
    }

    fn matches(&self, host: &HostProfile) -> bool {
        self.os
            .as_ref()
            .is_none_or(|requirement| requirement.matches(host))
            && self
                .features
                .iter()
                .all(|(name, required)| host.features.get(name) == Some(required))
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
struct PlatformRequirement {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    arch: Option<String>,
}

impl PlatformRequirement {
    fn matches(&self, host: &HostProfile) -> bool {
        if let Some(name) = &self.name {
            if let Some((platform, architecture)) = name.split_once('-') {
                if !host.platform.matches_name(platform)
                    || !host.architecture.matches_name(architecture)
                {
                    return false;
                }
            } else if !host.platform.matches_name(name) {
                return false;
            }
        }

        if let Some(pattern) = &self.version
            && !matches_entire(pattern, &host.platform_version)
        {
            return false;
        }
        if let Some(pattern) = &self.arch
            && !matches_entire(pattern, host.architecture.protocol_name())
        {
            return false;
        }
        true
    }
}

fn matches_entire(pattern: &str, value: &str) -> bool {
    Regex::new(pattern)
        .ok()
        .and_then(|regex| regex.find(value))
        .is_some_and(|matched| matched.start() == 0 && matched.end() == value.len())
}

#[cfg(test)]
mod tests {
    use super::{
        CompatibilityRule, HostProfile, MachineArchitecture, PlatformFamily, RuleDecision,
    };

    #[test]
    fn invalid_regular_expression_does_not_match() {
        let host = HostProfile::new(PlatformFamily::Windows, MachineArchitecture::X86_64, "10.0");
        let rules = [CompatibilityRule::new(RuleDecision::Permit).requiring_platform_version("[")];

        assert!(!host.allows(&rules));
    }
}
