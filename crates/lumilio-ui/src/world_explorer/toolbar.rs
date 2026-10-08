use super::MapView;
use crate::{controls::Segments, key::Key, tr, tr_all};
use gpui::{AnyElement, Context, div, prelude::*, px};
use gpui_component::{Sizable as _, h_flex, input::Input, select::Select, v_flex};
use lumilio_plugin_api::map::{Dimension, WorldId};

impl MapView {
    pub(super) fn toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let form = self.form.as_ref().unwrap();
        // ia[plugin.world-explorer]: 输入手动种子 | 地图顶部 · 种子输入框 | 数字或文字种子，Enter 或失焦应用；错误留在输入框旁；保存到该实例
        let seed = v_flex()
            .flex_1()
            .min_w(px(128.))
            .gap_1()
            .child(div().text_xs().child(tr!("map-seed")))
            .child(
                div()
                    .debug_selector(|| "map-seed-input".into())
                    .child(Input::new(&form.seed).small()),
            );
        // ia[plugin.world-explorer]: 选择种子版本 | 种子旁 · 可搜索版本下拉 | 默认世界的受支持版本，否则 1.21.4；确认后应用输入种子，不自动降级存档
        let version = v_flex()
            .w(px(128.))
            .gap_1()
            .child(div().text_xs().child(tr!("map-version")))
            .child(
                div().debug_selector(|| "map-version-select".into()).child(
                    Select::new(&form.version)
                        .small()
                        .accessibility_label(tr!("map-version"))
                        .search_placeholder(tr!("map-search-versions")),
                ),
            );
        // ia[plugin.world-explorer]: 选择世界 | 地图顶部 · 存档下拉 | 只列该实例存档；无存档时隐藏；选择后同步种子与版本，取消上一世界请求
        let world = self
            .contexts
            .iter()
            .any(|world| matches!(world.context.world, WorldId::Save { .. }))
            .then(|| {
                v_flex()
                    .flex_1()
                    .min_w(px(160.))
                    .gap_1()
                    .child(div().text_xs().child(tr!("map-world")))
                    .child(
                        div().debug_selector(|| "map-world-select".into()).child(
                            Select::new(&form.world)
                                .small()
                                .placeholder(tr!("map-choose-world"))
                                .accessibility_label(tr!("map-world"))
                                .search_placeholder(tr!("map-search-worlds")),
                        ),
                    )
            });
        let selected = match self.context.as_ref().map(|context| &context.dimension) {
            Some(Dimension::Nether) => 1,
            Some(Dimension::End) => 2,
            _ => 0,
        };
        let target = cx.weak_entity();
        // ia[plugin.world-explorer]: 切换维度 | 地图工具栏 · 维度分段 | 保留相机；清除旧维度帧和瓦片，取消旧请求
        let dimensions = Segments::new(
            "map-dimension",
            tr_all!["map-overworld", "map-nether", "map-end"],
            selected,
            move |index, _, cx| {
                let _ = target.update(cx, |this, cx| {
                    if let Some(context) = &mut this.context {
                        context.dimension =
                            [Dimension::Overworld, Dimension::Nether, Dimension::End][index]
                                .clone();
                    }
                    this.reset_view();
                    this.refresh();
                    cx.notify();
                });
            },
        );
        let labels: Vec<String> = self
            .providers
            .base_maps
            .iter()
            .map(|(_, base)| {
                crate::i18n::lookup(&base.kind_id).unwrap_or_else(|| base.kind_id.clone())
            })
            .collect();
        let target = cx.weak_entity();
        // ia[plugin.world-explorer]: 选择底图 | 地图工具栏 · 底图分段 | 单选；保留相机与维度，清除旧帧并取消旧底图请求
        let bases = Segments::new("map-base", &labels, self.base, move |index, _, cx| {
            let _ = target.update(cx, |this, cx| {
                this.base = index;
                this.reset_view();
                this.refresh();
                cx.notify();
            });
        });
        // ia[plugin.world-explorer]: 跳到坐标 | 地图工具栏 · 坐标输入框 | 输入 x z 或 x, z；Enter 移动中心；无效输入在框旁说明
        let jump = v_flex()
            .w(px(160.))
            .gap_1()
            .child(div().text_xs().child(tr!("map-jump")))
            .child(
                div()
                    .debug_selector(|| "map-jump-input".into())
                    .child(Input::new(&form.jump).small()),
            )
            .when(form.jump_error, |field| {
                field.child(
                    div()
                        .debug_selector(|| "map-jump-error".into())
                        .text_xs()
                        .child(tr!("map-coordinates-invalid")),
                )
            });
        v_flex()
            .w_full()
            .flex_none()
            .gap_2()
            .child(
                h_flex()
                    .w_full()
                    .gap_3()
                    .flex_wrap()
                    .items_end()
                    .child(seed)
                    .child(version)
                    .children(world),
            )
            .when_some(form.seed_error, |toolbar, error| {
                toolbar.child(
                    div()
                        .debug_selector(|| "map-seed-error".into())
                        .text_xs()
                        .child(if error == "map-seed-empty" {
                            tr!("map-seed-empty")
                        } else {
                            tr!("map-seed-save-failed")
                        }),
                )
            })
            .child(
                h_flex()
                    .gap_3()
                    .flex_wrap()
                    .items_end()
                    .child(dimensions)
                    .child(bases)
                    .child(jump),
            )
            .into_any_element()
    }

    pub(super) fn retry_button(&self, cx: &mut Context<Self>) -> Option<Key> {
        let count = self.failed.len();
        // ia[plugin.world-explorer]: 重试失败瓦片 | 地图状态行 ·「重试失败的 N 块」 | 一次重新派发全部失败块；没有失败时隐藏；暂停的来源重启后恢复
        (count > 0).then(|| {
            Key::new("map-retry")
                .label(tr!("map-retry-failed", count = count))
                .white()
                .small()
                .debug_selector(move || format!("map-retry-{count}"))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.failed.clear();
                    this.refresh();
                    cx.notify();
                }))
        })
    }
}
