use gpui_kit::component::{IconName, IconNamed as _};

use super::*;

/// Escaped once: every component icon rendered blank because our source
/// had no fallback (postmortem 0003).
#[test]
fn component_icons_and_our_own_both_load() {
    for name in [IconName::Check, IconName::Close, IconName::ChevronDown] {
        let path = name.path();
        assert!(
            AppAssets.load(&path).unwrap().is_some(),
            "component icon {path} does not load"
        );
    }
    assert!(
        AppAssets
            .load("icons/phosphor/house.svg")
            .unwrap()
            .is_some()
    );
    assert!(
        AppAssets
            .load("icons/nope.svg")
            .map_or(true, |data| data.is_none())
    );
}
