//! Host overlays use the same data geometry as plugin overlays.
use lumilio_plugin_api::map::{
    Dimension, MapObject, MapObjectKind, MapPoint, OverlayInfo, OverlayProvider, OverlayRequest,
};
use lumilio_plugin_api::{HostContext, PluginError};

pub struct UtilityOverlay;
impl OverlayProvider for UtilityOverlay {
    fn overlays(&self) -> Vec<OverlayInfo> {
        ["map-chunks", "map-regions", "map-coordinates"]
            .into_iter()
            .map(|id| OverlayInfo {
                id: id.into(),
                kind_id: id.into(),
                dimensions: vec![Dimension::Overworld, Dimension::Nether, Dimension::End],
                icon: None,
                group_id: None,
                approximate: false,
                max_scale: None,
            })
            .collect()
    }
    fn objects(
        &self,
        _: &dyn HostContext,
        request: &OverlayRequest,
    ) -> Result<Vec<MapObject>, PluginError> {
        let spacing = match request.overlay.as_str() {
            "map-regions" => 512.,
            "map-chunks" if request.level <= 1 => 16.,
            "map-chunks" | "map-coordinates" => return Ok(vec![]),
            _ => return Err(PluginError::InvalidInput("unknown utility overlay".into())),
        };
        let bounds = request.bounds;
        if [bounds.min.x, bounds.min.z, bounds.max.x, bounds.max.z]
            .iter()
            .any(|value| !value.is_finite() || value.abs() > 30_000_000.)
            || bounds.min.x > bounds.max.x
            || bounds.min.z > bounds.max.z
        {
            return Err(PluginError::InvalidInput("invalid utility bounds".into()));
        }
        let mut objects = Vec::new();
        for (axis, min, max) in [
            ("x", bounds.min.x, bounds.max.x),
            ("z", bounds.min.z, bounds.max.z),
        ] {
            let start = (min / spacing).ceil() as i64;
            let end = (max / spacing).floor() as i64;
            if end - start > 4096 {
                return Err(PluginError::InvalidInput("utility bounds too large".into()));
            }
            for index in start..=end {
                let at = index as f64 * spacing;
                let line = if axis == "x" {
                    vec![
                        MapPoint {
                            x: at,
                            z: bounds.min.z,
                        },
                        MapPoint {
                            x: at,
                            z: bounds.max.z,
                        },
                    ]
                } else {
                    vec![
                        MapPoint {
                            x: bounds.min.x,
                            z: at,
                        },
                        MapPoint {
                            x: bounds.max.x,
                            z: at,
                        },
                    ]
                };
                let raw_id = format!("{axis}:{index}");
                objects.push(MapObject {
                    id: format!("{}:{raw_id}", request.overlay),
                    raw_id,
                    source: "lumilio.host".into(),
                    world: request.context.world.clone(),
                    dimension: request.context.dimension.clone(),
                    kind: MapObjectKind::Polyline(line),
                    label: None,
                    label_id: None,
                    priority: 0,
                    approximate: false,
                    color: None,
                });
            }
        }
        Ok(objects)
    }
}
