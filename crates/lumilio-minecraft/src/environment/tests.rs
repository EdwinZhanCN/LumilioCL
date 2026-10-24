use super::{CompatibilityRule, HostProfile, MachineArchitecture, PlatformFamily, RuleDecision};

#[test]
fn invalid_regular_expression_does_not_match() {
    let host = HostProfile::new(PlatformFamily::Windows, MachineArchitecture::X86_64, "10.0");
    let rules = [CompatibilityRule::new(RuleDecision::Permit).requiring_platform_version("[")];

    assert!(!host.allows(&rules));
}
