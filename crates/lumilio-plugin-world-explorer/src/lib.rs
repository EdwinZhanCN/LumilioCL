//! Local seed-map provider. The host owns the world picker and map viewport.
use lumilio_cubiomes::Range;
use lumilio_plugin_api::map::{
    BaseMapInfo, BaseMapProvider, Dimension, MapObject, OverlayInfo, OverlayProvider,
    OverlayRequest, TILE_PIXELS, TileReply, TileRequest, WorldContext,
};
use lumilio_plugin_api::{
    API_VERSION, ActionId, Effect, GameFacts, HostContext, ImageData, InstanceTab, Manifest,
    Permission, Plugin, PluginError, TabState, View, Words,
};

mod landmarks;
mod structures;
mod text;
mod xaero;

pub const ID: &str = "lumilio.world-explorer";
pub const SUPPORTED_VERSIONS: &[&str] = lumilio_cubiomes::SUPPORTED_VERSIONS;
pub struct WorldExplorer;

impl Plugin for WorldExplorer {
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
                    under: "saves".into(),
                },
                Permission::ReadGameFiles {
                    under: "xaero".into(),
                },
            ],
            settings: vec![],
        }
    }
    fn instance_tab(&self) -> Option<&dyn InstanceTab> {
        Some(self)
    }
    fn base_map_provider(&self) -> Option<&dyn BaseMapProvider> {
        Some(self)
    }
    fn overlay_provider(&self) -> Option<&dyn OverlayProvider> {
        Some(self)
    }
}
impl OverlayProvider for WorldExplorer {
    fn overlays(&self) -> Vec<OverlayInfo> {
        let mut layers = structures::catalog();
        layers.extend(landmarks::catalog());
        layers.extend(xaero::overlay::catalog());
        layers
    }
    fn overlays_for(&self, context: &WorldContext) -> Vec<OverlayInfo> {
        let mut layers = structures::catalog_for(context);
        layers.extend(landmarks::catalog_for(context));
        layers.extend(xaero::overlay::catalog_for(context));
        layers
    }
    fn objects(
        &self,
        ctx: &dyn HostContext,
        request: &OverlayRequest,
    ) -> Result<Vec<MapObject>, PluginError> {
        if landmarks::is_landmark(&request.overlay) {
            landmarks::objects(request)
        } else if xaero::overlay::is_layer(&request.overlay) {
            xaero::overlay::objects(ctx, request)
        } else {
            structures::objects(ctx, request)
        }
    }
}
impl InstanceTab for WorldExplorer {
    fn title(&self) -> Words {
        text::tab_title()
    }
    // ia[plugin.world-explorer]: 打开地图 | 游戏页「插件」标签内的地图 | 宿主显示世界选择与地图；没有存档时可输入手动种子 | 插件默认启用，关闭后入口消失
    fn appears(&self, _: &GameFacts, _: &dyn HostContext) -> bool {
        true
    }
    fn view(&self, _: &dyn HostContext, _: &TabState) -> Result<View, PluginError> {
        Ok(View::Map)
    }
    fn update(
        &self,
        _: &dyn HostContext,
        state: TabState,
        _: ActionId,
    ) -> Result<(TabState, Vec<Effect>), PluginError> {
        Ok((state, vec![]))
    }
}
impl BaseMapProvider for WorldExplorer {
    fn base_maps(&self) -> Vec<BaseMapInfo> {
        vec![BaseMapInfo {
            id: "seed".into(),
            kind_id: "map-base-seed".into(),
            dimensions: vec![Dimension::Overworld, Dimension::Nether, Dimension::End],
            levels: vec![0, 1, 2, 3, 4],
        }]
    }
    fn tile(&self, ctx: &dyn HostContext, request: &TileRequest) -> Result<TileReply, PluginError> {
        if request.key.base_map != "seed"
            || request.key.provider != ID
            || request.pixels != TILE_PIXELS
            || request.context.world != request.key.world
            || request.context.dimension != request.key.dimension
        {
            return Err(PluginError::InvalidInput("invalid seed tile".into()));
        }
        let version = structures::world_version(&request.context)?;
        let Some(seed) = request.context.seed else {
            return Ok(TileReply::Empty);
        };
        let dimension = match request.key.dimension {
            Dimension::Overworld => lumilio_cubiomes::Dimension::Overworld,
            Dimension::Nether => lumilio_cubiomes::Dimension::Nether,
            Dimension::End => lumilio_cubiomes::Dimension::End,
            Dimension::Custom(_) => return Ok(TileReply::Empty),
        };
        let scale = request
            .key
            .blocks_per_pixel()
            .ok_or_else(|| PluginError::InvalidInput("invalid LOD".into()))?;
        let x = request
            .key
            .tx
            .checked_mul(256)
            .ok_or_else(|| PluginError::InvalidInput("tile outside border".into()))?;
        let z = request
            .key
            .tz
            .checked_mul(256)
            .ok_or_else(|| PluginError::InvalidInput("tile outside border".into()))?;
        if i64::from(x) * i64::from(scale) < -30_000_000
            || (i64::from(x) + 256) * i64::from(scale) > 30_000_000
            || i64::from(z) * i64::from(scale) < -30_000_000
            || (i64::from(z) + 256) * i64::from(scale) > 30_000_000
        {
            return Ok(TileReply::Empty);
        }
        let colors = lumilio_cubiomes::biome_colors();
        let mut rgba = Vec::with_capacity(256 * 256 * 4);
        let biomes = lumilio_cubiomes::biomes_until(
            version,
            seed,
            dimension,
            Range {
                scale,
                x,
                z,
                width: 256,
                height: 256,
            },
            || ctx.cancelled(),
        )
        .map_err(|error| match error {
            lumilio_cubiomes::Error::Cancelled => PluginError::Transient("map-cancelled".into()),
            _ => PluginError::InvalidInput("biome generation failed".into()),
        })?;
        for biome in biomes {
            let color = usize::try_from(biome)
                .ok()
                .and_then(|id| colors.get(id))
                .copied()
                .unwrap_or([128; 3]);
            rgba.extend([color[0], color[1], color[2], 255]);
        }
        Ok(TileReply::Image(ImageData {
            width: 256,
            height: 256,
            rgba,
        }))
    }
}

#[cfg(test)]
mod tests;
