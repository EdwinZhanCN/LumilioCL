//! The map in a window of its own: the key that opens it, what the person was
//! looking at (carried over), and the window's content.
use super::camera::Camera;
use super::layers::Layers;
use super::{Connection, MapView};
use crate::assets::UiIcon;
use crate::{key::Key, tr};
use gpui::{App, Context, Entity, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, TitleBar, v_flex};
use lumilio_plugin_api::map::{Dimension, WorldContext, WorldId};
use std::collections::BTreeSet;
use std::rc::Rc;

/// What the person was looking at, so the map in its own window opens on the
/// same world, dimension, place and layers. Opaque outside this module; the
/// application only carries it from the key to the window it opens.
#[derive(Clone, Debug, PartialEq)]
pub struct MapHandoff {
    place: Option<(WorldId, Dimension)>,
    camera: Camera,
    base: usize,
    chunks: bool,
    regions: bool,
    layers: BTreeSet<String>,
}

// The camera is clamped to finite values, so equality is reflexive.
impl Eq for MapHandoff {}

/// Opens the map in a window of its own, starting from this handoff.
pub type PopOut = Rc<dyn Fn(MapHandoff, &mut Window, &mut App)>;

impl MapView {
    /// Offers the key that opens the map in its own window. A map that is
    /// already in its own window is never given one.
    pub fn on_pop_out(mut self, pop_out: PopOut) -> Self {
        self.pop_out = Some(pop_out);
        self
    }

    pub fn handoff(&self) -> MapHandoff {
        MapHandoff {
            place: self
                .context
                .as_ref()
                .map(|context| (context.world.clone(), context.dimension.clone())),
            camera: self.camera,
            base: self.base,
            chunks: self.layers.chunks,
            regions: self.layers.regions,
            layers: self.objects.enabled.clone(),
        }
    }

    /// Starts from what another map showed. The camera, base map and layers
    /// apply at once; the world waits for the worlds to arrive
    /// ([`Self::initial_context`]).
    pub(super) fn restore(&mut self, handoff: MapHandoff) {
        self.camera = handoff.camera;
        self.base = handoff.base;
        self.layers = Layers {
            chunks: handoff.chunks,
            regions: handoff.regions,
        };
        self.objects.enabled = handoff.layers;
        self.restore = handoff.place;
    }

    /// The world to show once the worlds are known: the one carried over from
    /// another map if it still exists, otherwise the first.
    pub(super) fn initial_context(&mut self) -> Option<WorldContext> {
        let carried = self.restore.take().and_then(|(world, dimension)| {
            let mut context = self
                .contexts
                .iter()
                .find(|known| known.context.world == world)?
                .context
                .clone();
            context.dimension = dimension;
            Some(context)
        });
        carried.or_else(|| self.contexts.first().map(|world| world.context.clone()))
    }

    /// The key left of 图层, when the host can open the map's own window.
    pub(super) fn pop_out_key(&self, cx: &mut Context<Self>) -> Option<Key> {
        let pop_out = self.pop_out.clone()?;
        let target = cx.weak_entity();
        // ia[plugin.world-explorer]: 在新窗口打开地图 | 地图视口右下角 · 图层键左侧的全屏图标键 | 在独立窗口打开同一世界、维度、位置与图层，原视图不变；已在独立窗口里的地图不显示此键
        Some(
            Key::new("map-pop-out")
                .icon(Icon::new(UiIcon::Maximize))
                .white()
                .small()
                .tooltip(tr!("map-pop-out"))
                .on_click(move |_, window, cx| {
                    let Ok(handoff) = target.update(cx, |this, _| this.handoff()) else {
                        return;
                    };
                    let pop_out = pop_out.clone();
                    // The new window is opened after this click has finished.
                    window.defer(cx, move |window, cx| pop_out(handoff, window, cx));
                })
                .debug_selector(|| "map-pop-out".into()),
        )
    }
}

/// The content of the map's own window: a title bar over the map.
pub struct MapWindow {
    map: Entity<MapView>,
}

impl MapWindow {
    /// The map starts on `handoff` and talks to the application through
    /// `connection`, which belongs to this window alone.
    pub fn new(handoff: MapHandoff, connection: Connection, cx: &mut Context<Self>) -> Self {
        let map = cx.new(|cx| {
            let mut map = MapView::new(Rc::new(|_, _, _| {}), cx);
            map.restore(handoff);
            map
        });
        map.update(cx, |map, cx| map.connect(connection, cx));
        Self { map }
    }
}

impl Render for MapWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(TitleBar::new().child(div().text_sm().child(tr!("map-window-title"))))
            .child(div().flex_1().min_h_0().p(px(12.)).child(self.map.clone()))
    }
}
