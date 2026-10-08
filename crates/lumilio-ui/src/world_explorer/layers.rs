use super::MapView;
use crate::{controls::Fader, key::Key, tr};
use gpui::{Anchor, AnyElement, Context, div, prelude::*, px};
use gpui_component::{Sizable as _, h_flex, popover::Popover, v_flex};

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
            .w(px(260.))
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
            .into_any_element()
    }
}
