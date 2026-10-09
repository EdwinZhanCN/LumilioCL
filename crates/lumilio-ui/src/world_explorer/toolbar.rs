use super::MapView;
use crate::{controls::Segments, key::Key, tr, tr_all};
use gpui::{AnyElement, Context, div, prelude::*, px};
use gpui_component::{Sizable as _, h_flex, input::Input, select::Select, v_flex};
use lumilio_plugin_api::map::{Dimension, WorldId};

impl MapView {
    /// The seed row above the map: what world the map shows.
    pub(super) fn toolbar(&self) -> AnyElement {
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
        // ia[plugin.world-explorer]: 选择种子版本 | 种子旁 · 可搜索版本下拉 | 默认世界的受支持版本，否则最新的受支持版本（26.3）；确认后应用输入种子，不自动降级存档
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
            .any(|world| {
                matches!(
                    world.context.world,
                    WorldId::Save { .. } | WorldId::Server { .. }
                )
            })
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
        v_flex()
            .w_full()
            .flex_none()
            .gap_1()
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
            .into_any_element()
    }

    /// Dimension and base-map choices, floated over the map's top-left corner.
    pub(super) fn view_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let selected = match self.context.as_ref().map(|context| &context.dimension) {
            Some(Dimension::Nether) => 1,
            Some(Dimension::End) => 2,
            _ => 0,
        };
        let target = cx.weak_entity();
        // ia[plugin.world-explorer]: 切换维度 | 地图左上角 · 维度分段 | 保留相机；清除旧维度帧和瓦片，取消旧请求
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
                    this.context_changed();
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
        // ia[plugin.world-explorer]: 选择底图 | 地图左上角 · 底图分段 | 单选；保留相机与维度，清除旧帧并取消旧底图请求
        let bases = Segments::new("map-base", &labels, self.base, move |index, _, cx| {
            let _ = target.update(cx, |this, cx| {
                this.base = index;
                this.reset_view();
                this.refresh();
                cx.notify();
            });
        });
        h_flex()
            .gap_4()
            .items_start()
            .child(dimensions)
            .child(bases)
            .into_any_element()
    }

    /// X and Z fields with an explicit "Go" key, floated over the top-right corner.
    pub(super) fn jump_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let form = self.form.as_ref().unwrap();
        // ia[plugin.world-explorer]: 前往坐标 | 地图右上角 · X、Z 两个输入框与「前往」键 | 点「前往」或在任一框按 Enter 把视图中心移到该点；X 或 Z 无法读取时在框下说明，视图不动
        v_flex()
            .gap_1()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .w(px(96.))
                            .debug_selector(|| "map-jump-x".into())
                            .child(Input::new(&form.jump_x).small()),
                    )
                    .child(
                        div()
                            .w(px(96.))
                            .debug_selector(|| "map-jump-z".into())
                            .child(Input::new(&form.jump_z).small()),
                    )
                    .child(
                        Key::new("map-go")
                            .label(tr!("map-go"))
                            .white()
                            .small()
                            .debug_selector(|| "map-go".into())
                            .on_click(cx.listener(|this, _, _, cx| this.jump(cx))),
                    ),
            )
            .when(form.jump_error, |field| {
                field.child(
                    div()
                        .debug_selector(|| "map-jump-error".into())
                        .text_xs()
                        .child(tr!("map-coordinates-invalid")),
                )
            })
            .into_any_element()
    }

    pub(super) fn retry_button(&self, cx: &mut Context<Self>) -> Option<Key> {
        let count = self.failed.len() + self.objects.failed();
        // ia[plugin.world-explorer]: 重试失败瓦片 | 地图左下角状态条 ·「重试失败的 N 块」 | 一次重新派发全部失败块；没有失败时隐藏；暂停的来源重启后恢复
        (count > 0).then(|| {
            Key::new("map-retry")
                .label(tr!("map-retry-failed", count = count))
                .white()
                .small()
                .debug_selector(move || format!("map-retry-{count}"))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.failed.clear();
                    this.objects.retry();
                    this.refresh();
                    cx.notify();
                }))
        })
    }
}
