//! The application's asset source: LumilioCL's own icons first, then the
//! component library's bundled icons (check marks, dialog close, chevrons…),
//! which gpui-component draws by path and which our source does not carry.

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match lumilio_ui::assets::Assets.load(path)? {
            Some(data) => Ok(Some(data)),
            None => gpui_kit::assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut all = lumilio_ui::assets::Assets.list(path)?;
        all.extend(gpui_kit::assets::Assets.list(path)?);
        all.sort();
        all.dedup();
        Ok(all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::component::{IconName, IconNamed as _};

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
}
