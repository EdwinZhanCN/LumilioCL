use super::MapView;
use crate::{controls::Fader, key::Key, kit, tr};
use gpui::SharedString;
use gpui::{Anchor, AnyElement, Context, ObjectFit, div, img, prelude::*, px};
use gpui_component::{ActiveTheme as _, Sizable as _, h_flex, popover::Popover, v_flex};
use lumilio_plugin_api::map::OverlayInfo;

#[derive(Clone, Copy, Default)]
pub(super) struct Layers {
    pub chunks: bool,
    pub regions: bool,
}

impl MapView {
    pub(super) fn layer_popover(&self, cx: &mut Context<Self>) -> Popover {
        let target = cx.weak_entity();
        // ia[plugin.world-explorer]: 打开图层 | 地图视口右下角 ·「图层」 | 打开次要图层浮层；Escape 或点击外部关闭并返回焦点
        Popover::new("map-layer-popover")
            .anchor(Anchor::BottomRight)
            .trigger(
                Key::new("map-layers")
                    .label(tr!("map-layers"))
                    .white()
                    .small()
                    .debug_selector(|| "map-layers".into()),
            )
            .content(move |_, _, cx| {
                target
                    .update(cx, |this, cx| this.layer_controls(cx))
                    .unwrap_or_else(|_| div().into_any_element())
            })
    }

    fn layer_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        // ia[plugin.world-explorer]: 显示区块网格 | 图层浮层 · 区块网格开关 | 即刻开关；每像素超过 4 方块时隐藏细线，并在开关旁说明
        let chunks = Fader::new("map-chunks", self.layers.chunks, tr!("map-chunks"), {
            let target = cx.weak_entity();
            move |_, cx| {
                let _ = target.update(cx, |this, cx| {
                    this.layers.chunks = !this.layers.chunks;
                    this.frame();
                    cx.notify();
                });
            }
        });
        // ia[plugin.world-explorer]: 显示 Region 边界 | 图层浮层 · Region 边界开关 | 即刻开关 512 方块边界；不改变底图或相机
        let regions = Fader::new("map-regions", self.layers.regions, tr!("map-regions"), {
            let target = cx.weak_entity();
            move |_, cx| {
                let _ = target.update(cx, |this, cx| {
                    this.layers.regions = !this.layers.regions;
                    this.frame();
                    cx.notify();
                });
            }
        });
        v_flex()
            .w(px(280.))
            .gap_3()
            .debug_selector(|| "map-layer-panel".into())
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .child(tr!("map-chunks"))
                    .child(
                        div()
                            .debug_selector(|| "map-chunks-toggle".into())
                            .child(chunks),
                    ),
            )
            .when(self.camera.scale > 4., |panel| {
                panel.child(
                    div()
                        .text_xs()
                        .debug_selector(|| "map-chunks-hint".into())
                        .child(tr!("map-chunks-hidden")),
                )
            })
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .child(tr!("map-regions"))
                    .child(
                        div()
                            .debug_selector(|| "map-regions-toggle".into())
                            .child(regions),
                    ),
            )
            .children(self.overlay_groups(cx))
            .into_any_element()
    }

    /// Plugin layers grouped under their titles, in the order the plugin lists
    /// them. A long group scrolls inside the panel rather than growing it.
    fn overlay_groups(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.objects.layers.is_empty() {
            return None;
        }
        type Group<'a> = (Option<&'a str>, Vec<&'a (String, OverlayInfo)>);
        let mut groups: Vec<Group> = Vec::new();
        for layer in &self.objects.layers {
            let group = layer.1.group_id.as_deref();
            match groups.iter_mut().find(|(known, _)| *known == group) {
                Some((_, rows)) => rows.push(layer),
                None => groups.push((group, vec![layer])),
            }
        }
        let estimated = self
            .objects
            .layers
            .iter()
            .any(|(_, layer)| layer.approximate);
        // ia[plugin.world-explorer]: 显示或隐藏结构图层 | 图层浮层 · 按种类分组的开关 | 即刻开关，图标按世界版本与维度出现；每像素超过 16 方块时隐藏并说明；1.18 起沙漠神殿、丛林神殿、林地府邸标「估计」
        let rows =
            groups.into_iter().map(|(group, rows)| {
                v_flex()
                    .w_full()
                    .gap_2()
                    .children(group.map(|group| {
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(crate::i18n::lookup(group).unwrap_or_else(|| group.to_owned()))
                    }))
                    .children(rows.into_iter().map(|(_, layer)| {
                        let id = layer.id.clone();
                        let title = crate::i18n::lookup(&layer.kind_id)
                            .unwrap_or_else(|| layer.kind_id.clone());
                        let target = cx.weak_entity();
                        let toggle = {
                            let id = id.clone();
                            move |_: &mut gpui::Window, cx: &mut gpui::App| {
                                let _ = target.update(cx, |this, cx| {
                                    this.objects.toggle(&id);
                                    this.refresh_objects();
                                    this.frame();
                                    cx.notify();
                                });
                            }
                        };
                        h_flex()
                            .w_full()
                            .gap_2()
                            .items_center()
                            .debug_selector({
                                let id = id.clone();
                                move || format!("map-layer-row-{id}")
                            })
                            .child(div().flex_none().size(px(20.)).children(
                                layer.icon.and_then(crate::map_icons::image).map(|image| {
                                    img(image).size_full().object_fit(ObjectFit::Contain)
                                }),
                            ))
                            .child(div().flex_1().min_w_0().truncate().child(title.clone()))
                            .when(layer.approximate, |row| {
                                row.child(
                                    div()
                                        .flex_none()
                                        .text_xs()
                                        .debug_selector({
                                            let id = id.clone();
                                            move || format!("map-layer-estimated-{id}")
                                        })
                                        .child(tr!("map-estimated")),
                                )
                            })
                            .child(
                                div()
                                    .flex_none()
                                    .debug_selector({
                                        let id = id.clone();
                                        move || format!("map-layer-toggle-{id}")
                                    })
                                    .child(Fader::new(
                                        SharedString::from(format!("map-layer-{id}")),
                                        self.objects.enabled.contains(&id),
                                        title,
                                        toggle,
                                    )),
                            )
                    }))
            });
        let list = kit::keep_wheel(
            v_flex()
                .id("map-layer-list")
                .w_full()
                .max_h(px(320.))
                .gap_3()
                .overflow_y_scroll()
                .track_scroll(&self.layer_scroll)
                .children(rows),
            &self.layer_scroll,
        );
        Some(
            v_flex()
                .w_full()
                .gap_2()
                .child(list)
                .when(self.camera.scale > super::objects::MAX_SCALE, |panel| {
                    panel.child(
                        div()
                            .text_xs()
                            .debug_selector(|| "map-structures-hint".into())
                            .child(tr!("map-structures-hidden")),
                    )
                })
                .when(estimated, |panel| {
                    panel.child(
                        div()
                            .text_xs()
                            .debug_selector(|| "map-estimated-note".into())
                            .child(tr!("map-estimated-note")),
                    )
                })
                .into_any_element(),
        )
    }
}
