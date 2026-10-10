//! Lists the `.litematic` files in a game's `schematics` folder, with their
//! details and material lists. All file access goes through the host.

mod format;
mod overlay;
mod tab;
#[cfg(test)]
mod tests;
mod text;

use lumilio_plugin_api::{
    API_VERSION, GameFacts, HostContext, InstanceTab, Manifest, Permission, Plugin, PluginError,
    TabState, View, Words,
};

pub const ID: &str = "lumilio.litematica";
const FOLDER: &str = "schematics";

pub struct Litematica;

impl Plugin for Litematica {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: ID.into(),
            name: text::name(),
            description: text::description(),
            version: env!("CARGO_PKG_VERSION").into(),
            api: API_VERSION,
            default_enabled: true,
            permissions: vec![
                Permission::ReadGameFiles {
                    under: FOLDER.into(),
                },
                Permission::ReadGameFiles {
                    under: "config/litematica".into(),
                },
            ],
            settings: Vec::new(),
        }
    }

    fn instance_tab(&self) -> Option<&dyn InstanceTab> {
        Some(self)
    }

    fn overlay_provider(&self) -> Option<&dyn lumilio_plugin_api::map::OverlayProvider> {
        Some(self)
    }
}

impl InstanceTab for Litematica {
    fn title(&self) -> Words {
        text::tab_title()
    }

    /// The game has Litematica, or already has files in `schematics/`.
    // ia[instance]: 出现「投影」标签 | 游戏页「插件」标签内的左侧列表 | 装了 Litematica 或 schematics 文件夹里已有文件时出现；在设置 → 插件里关掉就消失 | 空的 schematics 文件夹不算（插件只能列出文件）
    fn appears(&self, game: &GameFacts, ctx: &dyn HostContext) -> bool {
        game.mods.iter().any(|item| item.id == "litematica")
            || ctx.list_files(FOLDER).is_ok_and(|files| !files.is_empty())
    }

    fn view(&self, ctx: &dyn HostContext, state: &TabState) -> Result<View, PluginError> {
        tab::view(ctx, state)
    }

    fn update(
        &self,
        ctx: &dyn HostContext,
        state: TabState,
        action: lumilio_plugin_api::ActionId,
    ) -> Result<(TabState, Vec<lumilio_plugin_api::Effect>), PluginError> {
        tab::update(ctx, state, &action)
    }
}
