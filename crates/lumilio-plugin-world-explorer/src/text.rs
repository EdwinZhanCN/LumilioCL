//! The plugin's own words, Chinese and English side by side.

use lumilio_plugin_api::Words;

pub(crate) fn name() -> Words {
    Words::new("世界地图", "World Explorer")
}

pub(crate) fn description() -> Words {
    Words::new(
        "在本机查看存档与种子地图。",
        "Browse local worlds and seed maps.",
    )
}

pub(crate) fn tab_title() -> Words {
    Words::new("地图", "Map")
}
