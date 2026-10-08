//! Modrinth plugin words, Chinese and English side by side. Modrinth itself
//! is a proper noun and stays as it is.

use lumilio_plugin_api::Words;

pub(crate) fn name() -> Words {
    Words::new("Modrinth", "Modrinth")
}

pub(crate) fn description() -> Words {
    Words::new(
        "查找、安装和更新 Mod、整合包、资源包与光影。",
        "Find, install and update mods, modpacks, resource packs and shaders.",
    )
}
