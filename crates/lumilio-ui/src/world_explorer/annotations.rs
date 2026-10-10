//! Launcher-owned markers, routes and measurement controls.
use super::{Command, MapView, edit};
use crate::{key::Key, tr};
use gpui::{Anchor, AnyElement, Context, SharedString, Window, div, prelude::*, px};
use gpui_component::{Sizable as _, WindowExt as _, h_flex, popover::Popover, v_flex};
use lumilio_core::world_map::store::annotations::{Annotation, AnnotationKind, route_length};
use lumilio_plugin_api::map::MapPoint;
use lumilio_plugin_api::{SettingField, SettingKind, SettingValue, Words};
use std::collections::BTreeMap;
use std::rc::Rc;

fn field(key: &str, label: &str, kind: SettingKind) -> SettingField {
    SettingField {
        key: key.into(),
        // The host creates these fields in the active locale; a locale switch
        // recreates the map view and its dialog.
        label: Words::new(label, label),
        help: Words::default(),
        kind,
    }
}

fn color(index: usize) -> [u8; 3] {
    let value = edit::SWATCHES[index];
    [(value >> 16) as u8, (value >> 8) as u8, value as u8]
}

impl MapView {
    pub(super) fn bookmark_key(&self, cx: &mut Context<Self>) -> Option<Key> {
        let selected = self.selected.as_ref()?;
        (selected.source != "lumilio.launcher" && super::select::position(selected).is_some()).then(
            || {
                // ia[plugin.world-explorer]: 把地图对象保存为标记 | 地图对象信息卡 ·「保存标记」 | 保留原来源关联和坐标；原对象消失时标记仍在
                Key::new("map-bookmark")
                    .label(tr!("map-bookmark"))
                    .white()
                    .small()
                    .on_click(cx.listener(|this, _, window, cx| {
                        let Some(object) = this.selected.as_ref() else {
                            return;
                        };
                        let Some(at) = super::select::position(object) else {
                            return;
                        };
                        let item = Annotation {
                            id: 0,
                            world: object.world.clone(),
                            dimension: object.dimension.clone(),
                            kind: AnnotationKind::Marker,
                            name: object
                                .label
                                .clone()
                                .unwrap_or_else(|| object.raw_id.clone()),
                            color: object.color.unwrap_or_else(|| color(6)),
                            points: vec![at],
                            linked_source: Some(object.source.clone()),
                            linked_raw_id: Some(object.raw_id.clone()),
                        };
                        this.open_annotation_dialog(
                            AnnotationKind::Marker,
                            vec![at],
                            Some(item),
                            window,
                            cx,
                        );
                    }))
            },
        )
    }

    pub(super) fn annotation_keys(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let selected = self.selected.as_ref()?;
        if selected.source != "lumilio.launcher" {
            return None;
        }
        let id = selected.raw_id.parse::<i64>().ok()?;
        self.annotations.iter().find(|item| item.id == id)?;
        // ia[plugin.world-explorer]: 编辑或删除标记路线 | 地图对象信息卡 ·「编辑」「删除」 | 编辑名称、颜色；删除前确认，随后刷新该世界的标记和路线
        Some(
            h_flex()
                .gap_2()
                .child(
                    Key::new("map-annotation-edit")
                        .label(tr!("common-edit"))
                        .white()
                        .small()
                        .on_click(cx.listener(|this, _, window, cx| {
                            let Some(id) = this
                                .selected
                                .as_ref()
                                .and_then(|o| o.raw_id.parse::<i64>().ok())
                            else {
                                return;
                            };
                            let Some(item) =
                                this.annotations.iter().find(|item| item.id == id).cloned()
                            else {
                                return;
                            };
                            this.open_annotation_dialog(
                                item.kind,
                                item.points.clone(),
                                Some(item),
                                window,
                                cx,
                            );
                        })),
                )
                .child(
                    Key::new("map-annotation-delete")
                        .label(tr!("common-delete"))
                        .white()
                        .small()
                        .on_click(cx.listener(|this, _, window, cx| {
                            let Some(id) = this
                                .selected
                                .as_ref()
                                .and_then(|o| o.raw_id.parse::<i64>().ok())
                            else {
                                return;
                            };
                            let Some(item) = this.annotations.iter().find(|item| item.id == id)
                            else {
                                return;
                            };
                            let name = item.name.clone();
                            let target = cx.weak_entity();
                            window.open_alert_dialog(cx, move |alert, _, _| {
                                let target = target.clone();
                                alert
                                    .title(tr!("map-annotation-delete-title", name = name.clone()))
                                    .ok_text(tr!("common-delete"))
                                    .ok_variant(gpui_component::button::ButtonVariant::Danger)
                                    .cancel_text(tr!("common-cancel"))
                                    .show_cancel(true)
                                    .on_ok(move |_, _, cx| {
                                        let _ = target.update(cx, |this, cx| {
                                            if let (Some(context), Some(connection)) =
                                                (&this.context, &this.connection)
                                            {
                                                let _ = connection.send.try_send(
                                                    Command::RemoveAnnotation {
                                                        context: context.clone(),
                                                        id,
                                                    },
                                                );
                                            }
                                            this.selected = None;
                                            cx.notify();
                                        });
                                        true
                                    })
                            });
                        })),
                )
                .into_any_element(),
        )
    }

    pub(super) fn annotation_controls(&self, cx: &mut Context<Self>) -> Popover {
        let target = cx.weak_entity();
        // ia[plugin.world-explorer]: 打开地图工具 | 地图右下角 ·「工具」 | 展开标记、路线和测距操作
        Popover::new("map-tools-popover")
            .anchor(Anchor::BottomRight)
            .trigger(
                Key::new("map-tools")
                    .label(tr!("map-tools"))
                    .white()
                    .small()
                    .debug_selector(|| "map-tools".into()),
            )
            .content(move |_, _, cx| {
                target
                    .update(cx, |this, cx| this.annotation_panel(cx))
                    .unwrap_or_else(|_| div().into_any_element())
            })
    }

    fn annotation_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        // ia[plugin.world-explorer]: 放置自定义标记 | 地图右下角 ·「添加标记」 | 下一次点击地图的位置创建该世界、维度的标记，可编辑名称和颜色
        let marker = Key::new("map-add-marker")
            .label(tr!("map-add-marker"))
            .white()
            .small()
            .disabled(self.context.is_none())
            .debug_selector(|| "map-add-marker".into())
            .on_click(cx.listener(|this, _, _, cx| {
                this.marking = true;
                this.routing = false;
                this.route_points.clear();
                this.measure_mode = false;
                cx.notify();
            }));
        // ia[plugin.world-explorer]: 绘制路线 | 地图右下角 ·「绘制路线」和「完成」 | 依次点击至少两个点，完成后填写名称和颜色；显示总长度
        let route = Key::new("map-draw-route")
            .label(tr!("map-draw-route"))
            .white()
            .small()
            .disabled(self.context.is_none())
            .debug_selector(|| "map-draw-route".into())
            .on_click(cx.listener(|this, _, _, cx| {
                this.routing = !this.routing;
                this.route_points.clear();
                this.marking = false;
                this.measure_mode = false;
                cx.notify();
            }));
        // ia[plugin.world-explorer]: 测量两点距离 | 地图右下角 ·「测距」 | 依次点两处，显示方块距离；结果不保存
        let measure = Key::new("map-measure")
            .label(tr!("map-measure"))
            .white()
            .small()
            .disabled(self.context.is_none())
            .debug_selector(|| "map-measure".into())
            .on_click(cx.listener(|this, _, _, cx| {
                this.measure_mode = true;
                this.measure_start = None;
                this.measure_end = None;
                this.routing = false;
                this.route_points.clear();
                this.marking = false;
                cx.notify();
            }));
        v_flex()
            .w(px(180.))
            .gap_2()
            .child(marker)
            .child(route)
            .child(measure)
            .children((self.routing && self.route_points.len() >= 2).then(|| {
                Key::new("map-finish-route")
                    .label(tr!("map-finish-route"))
                    .white()
                    .small()
                    .debug_selector(|| "map-finish-route".into())
                    .on_click(cx.listener(|this, _, window, cx| {
                        let points = std::mem::take(&mut this.route_points);
                        this.routing = false;
                        this.open_annotation_dialog(
                            AnnotationKind::Route,
                            points,
                            None,
                            window,
                            cx,
                        );
                        cx.notify();
                    }))
            }))
            .into_any_element()
    }

    pub(super) fn annotation_hint(&self) -> Option<String> {
        if self.marking {
            Some(tr!("map-marker-hint").into())
        } else if self.routing {
            Some(tr!("map-route-hint", count = self.route_points.len()))
        } else if self.measure_mode {
            Some(tr!("map-measure-hint").into())
        } else if let (Some(start), Some(end)) = (self.measure_start, self.measure_end) {
            Some(tr!(
                "map-measure-result",
                length = route_length(&[start, end]).round() as i64
            ))
        } else {
            None
        }
    }

    pub(super) fn map_annotation_click(
        &mut self,
        at: MapPoint,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.marking {
            self.marking = false;
            self.open_annotation_dialog(AnnotationKind::Marker, vec![at], None, window, cx);
        } else if self.routing {
            self.route_points.push(at);
            self.frame();
        } else if self.measure_mode {
            if let Some(start) = self.measure_start {
                self.measure_end = Some(at);
                self.measure_start = Some(start);
                self.measure_mode = false;
            } else {
                self.measure_start = Some(at);
            }
        }
    }

    pub(super) fn open_annotation_dialog(
        &mut self,
        kind: AnnotationKind,
        points: Vec<MapPoint>,
        existing: Option<Annotation>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(context) = self.context.clone() else {
            return;
        };
        let name = existing.as_ref().map_or("", |item| item.name.as_str());
        let selected = existing
            .as_ref()
            .and_then(|item| (0..edit::SWATCHES.len()).find(|&index| color(index) == item.color))
            .unwrap_or(6);
        let fields = vec![
            field(
                "name",
                tr!("map-annotation-name"),
                SettingKind::Text {
                    default: name.into(),
                },
            ),
            field(
                "color",
                tr!("map-annotation-color"),
                SettingKind::Choice {
                    options: (0..edit::SWATCHES.len())
                        .map(|index| index.to_string())
                        .collect(),
                    default: selected.to_string(),
                },
            ),
        ];
        let target = cx.weak_entity();
        let submit: edit::Submit = Rc::new(move |values: BTreeMap<String, SettingValue>, _, cx| {
            let name = match values.get("name") {
                Some(SettingValue::Text(name))
                    if !name.trim().is_empty() && name.chars().count() <= 64 =>
                {
                    name.trim().to_owned()
                }
                _ => {
                    let _ = target.update(cx, |this, _| {
                        this.applied = Some(Err("map-annotation-invalid-name".into()))
                    });
                    return;
                }
            };
            let selected = match values.get("color") {
                Some(SettingValue::Choice(index)) => index
                    .parse::<usize>()
                    .ok()
                    .filter(|index| *index < edit::SWATCHES.len())
                    .unwrap_or(6),
                _ => 6,
            };
            let item = Annotation {
                id: existing.as_ref().map_or(0, |item| item.id),
                world: context.world.clone(),
                dimension: context.dimension.clone(),
                kind,
                name,
                color: color(selected),
                points: points.clone(),
                linked_source: existing
                    .as_ref()
                    .and_then(|item| item.linked_source.clone()),
                linked_raw_id: existing
                    .as_ref()
                    .and_then(|item| item.linked_raw_id.clone()),
            };
            let _ = target.update(cx, |this, _| {
                if this.connection.as_ref().is_some_and(|connection| {
                    connection
                        .send
                        .try_send(Command::PutAnnotation {
                            context: context.clone(),
                            annotation: Box::new(item),
                        })
                        .is_ok()
                }) {
                    this.annotation_pending = true;
                } else {
                    this.applied = Some(Err("map-annotation-save-failed".into()));
                }
            });
        });
        let title: SharedString = match kind {
            AnnotationKind::Marker => tr!("map-annotation-marker"),
            AnnotationKind::Route => tr!("map-annotation-route"),
        }
        .into();
        let dialog = edit::EditDialog::open(title, fields, BTreeMap::new(), submit, window, cx);
        self.edit_dialog = Some(dialog.downgrade());
    }
}
