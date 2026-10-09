//! Picking an overlay object on the map and showing what it is.
use super::{ICON, MAX_ICONS, MapView, canvas};
use crate::{key::Key, tr};
use gpui::{AnyElement, ClipboardItem, Context, Pixels, Point, div, prelude::*, px};
use gpui_component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use lumilio_plugin_api::map::{MapObject, MapObjectKind, MapPoint};

/// A press that moves less than this many pixels before release is a click,
/// not the start of a pan.
const CLICK_SLOP: f32 = 4.;
/// How far outside an icon's box a click still picks it, in pixels.
const REACH: f64 = 4.;

/// Where an object sits on the map, when it has one point to name.
pub(super) fn position(object: &MapObject) -> Option<MapPoint> {
    match &object.kind {
        MapObjectKind::Icon { at, .. } | MapObjectKind::Point(at) => Some(*at),
        MapObjectKind::Text { at, .. } => Some(*at),
        _ => None,
    }
}

/// The text "copy coordinates" puts on the clipboard: `x z`, whole blocks.
pub(super) fn coordinates(at: MapPoint) -> String {
    format!("{} {}", at.x.round() as i64, at.z.round() as i64)
}

/// The built-in plugin's id, whose objects come from the seed.
const SEED_SOURCE: &str = "lumilio.world-explorer";

/// What the viewer calls a kind of source, in the language of the interface.
fn source_name(source: &str) -> String {
    if source == SEED_SOURCE {
        tr!("map-source-seed").to_string()
    } else {
        source.to_owned()
    }
}

impl MapView {
    /// The object whose icon is under `local`, in viewport pixels. Icons that
    /// draw later sit on top, so they win a tie.
    pub(super) fn pick(&self, local: [f64; 2]) -> Option<MapObject> {
        let (width, height) = (f64::from(self.size[0]), f64::from(self.size[1]));
        let objects = self.objects.visible(self.view_blocks());
        let skip = objects.len().saturating_sub(MAX_ICONS);
        let half = f64::from(ICON) / 2. + REACH;
        objects
            .into_iter()
            .skip(skip)
            .filter_map(|object| {
                let MapObjectKind::Icon { at, .. } = &object.kind else {
                    return None;
                };
                let x = (at.x - self.camera.x) / self.camera.scale + width / 2.;
                let y = (at.z - self.camera.z) / self.camera.scale + height / 2.;
                let (dx, dy) = ((x - local[0]).abs(), (y - local[1]).abs());
                (dx <= half && dy <= half).then(|| (dx.max(dy), object))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, object)| object.clone())
    }

    /// The block position of a click: a release near its press, in world
    /// coordinates; `None` when the pointer moved far enough to be a pan.
    pub(super) fn click_point(&self, down: Point<Pixels>, at: Point<Pixels>) -> Option<[f64; 2]> {
        let moved = at - down;
        if f32::from(moved.x).hypot(f32::from(moved.y)) > CLICK_SLOP {
            return None;
        }
        let local = at - self.bounds?.origin;
        Some(self.camera.world(
            [f32::from(local.x) as f64, f32::from(local.y) as f64],
            self.size,
        ))
    }

    /// A release at `at` after a press at `down` selects the object under it,
    /// or clears the selection over empty map.
    pub(super) fn click(&mut self, down: Point<Pixels>, at: Point<Pixels>) {
        let moved = at - down;
        if f32::from(moved.x).hypot(f32::from(moved.y)) > CLICK_SLOP {
            return;
        }
        let Some(bounds) = self.bounds else { return };
        let local = at - bounds.origin;
        self.selected = self.pick([f32::from(local.x) as f64, f32::from(local.y) as f64]);
        if self
            .selected
            .as_ref()
            .is_some_and(|o| !o.editable.is_empty())
        {
            self.probe_edit();
        }
    }

    /// A soft square behind the selected icon, painted under the icons.
    pub(super) fn selection_fill(&self) -> Option<canvas::Fill> {
        let at = position(self.selected.as_ref()?)?;
        let (width, height) = (f64::from(self.size[0]), f64::from(self.size[1]));
        let x = (at.x - self.camera.x) / self.camera.scale + width / 2.;
        let y = (at.z - self.camera.z) / self.camera.scale + height / 2.;
        let half = f64::from(ICON) / 2. + 3.;
        Some(canvas::Fill {
            rect: [x - half, y - half, x + half, y + half],
            rgba: [255, 255, 255, 150],
        })
    }

    /// The card for the selected object: what it is, where, and who says so.
    pub(super) fn selection_card(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let object = self.selected.as_ref()?;
        let at = position(object)?;
        let title = object
            .label
            .clone()
            .or_else(|| {
                object
                    .label_id
                    .as_deref()
                    .map(|id| crate::i18n::lookup(id).unwrap_or_else(|| id.to_owned()))
            })
            .unwrap_or_else(|| object.raw_id.clone());
        let text = coordinates(at);
        let share = object.share.clone();
        // What the object is, when its title is the user's own name for it.
        let kind = object
            .label
            .as_ref()
            .and(object.label_id.as_deref())
            .map(|id| crate::i18n::lookup(id).unwrap_or_else(|| id.to_owned()));
        // ia[plugin.world-explorer]: 点选地图对象 | 地图右下角信息卡 · 种类、坐标、来源与「复制坐标」 | 点图标选中并显示；点空白处或「×」取消；复制 `x z`；估计的位置标「估计」
        Some(
            v_flex()
                .gap_1()
                .min_w(px(180.))
                .debug_selector(|| "map-selection".into())
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(div().flex_1().min_w_0().truncate().child(title))
                        .when(object.approximate, |row| {
                            row.child(
                                div()
                                    .flex_none()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(tr!("map-estimated")),
                            )
                        }),
                )
                .children(kind.map(|kind| {
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(kind)
                }))
                .children(object.note.clone().map(|note| {
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .debug_selector(|| "map-selection-note".into())
                        .child(note)
                }))
                .child(
                    div()
                        .debug_selector(|| "map-selection-coordinates".into())
                        .child(format!(
                            "X {}  Z {}",
                            at.x.round() as i64,
                            at.z.round() as i64
                        )),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(tr!(
                            "map-selection-source",
                            source = source_name(&object.source)
                        )),
                )
                .children(self.edit_keys(cx))
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Key::new("map-copy-coordinates")
                                .label(tr!("map-copy-coordinates"))
                                .white()
                                .small()
                                .debug_selector(|| "map-copy-coordinates".into())
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                                })),
                        )
                        .children(share.map(|share| {
                            // ia[plugin.world-explorer]: 复制路径点分享串 | 信息卡 ·「复制分享串」 | 复制 Xaero 的 xaero-waypoint: 格式，可在游戏里导入；只在路径点上出现
                            Key::new("map-copy-share")
                                .label(tr!("map-copy-share"))
                                .white()
                                .small()
                                .debug_selector(|| "map-copy-share".into())
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(share.clone()));
                                }))
                        }))
                        .child(
                            Key::new("map-selection-close")
                                .label(tr!("map-selection-close"))
                                .white()
                                .small()
                                .debug_selector(|| "map-selection-close".into())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.selected = None;
                                    cx.notify();
                                })),
                        ),
                )
                .into_any_element(),
        )
    }
}
