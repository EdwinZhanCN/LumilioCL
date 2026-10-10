use super::MapView;
use crate::{key::Key, kit, theme, tr, tr_all};
use gpui::{AnyElement, ClipboardItem, Context, div, prelude::*, px};
use gpui_component::{Sizable as _, h_flex, input::Input, select::Select, v_flex};
use lumilio_core::world_map::store::annotations::portal_coordinates;
use lumilio_plugin_api::map::{Dimension, MapPoint, WorldId};

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
            .when_some(form.seed_error, |toolbar, _| {
                toolbar.child(
                    div()
                        .debug_selector(|| "map-seed-error".into())
                        .text_xs()
                        .child(tr!("map-seed-save-failed")),
                )
            })
            .into_any_element()
    }

    /// The dimension and the place key, floated over the map's top-left corner.
    pub(super) fn view_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let selected = match self.context.as_ref().map(|context| &context.dimension) {
            Some(Dimension::Nether) => 1,
            Some(Dimension::End) => 2,
            _ => 0,
        };
        let target = cx.weak_entity();
        // ia[plugin.world-explorer]: 切换维度 | 地图左上角 · 维度标签 | 保留相机；清除旧维度帧和瓦片，取消旧请求
        let dimensions = kit::tabs(
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
        h_flex()
            .gap_3()
            .items_center()
            .child(dimensions)
            .children(self.place_key(cx))
            .into_any_element()
    }

    /// The kind of map, floated over the map's top-right corner.
    pub(super) fn base_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let labels: Vec<String> = self
            .providers
            .base_maps
            .iter()
            .map(|(_, base)| {
                crate::i18n::lookup(&base.kind_id).unwrap_or_else(|| base.kind_id.clone())
            })
            .collect();
        let target = cx.weak_entity();
        // ia[plugin.world-explorer]: 选择底图 | 地图右上角 · 底图标签 | 单选；保留相机与维度，清除旧帧并取消旧底图请求
        let bases = kit::tabs("map-base", &labels, self.base, move |index, _, cx| {
            let _ = target.update(cx, |this, cx| {
                this.base = index;
                this.reset_view();
                this.refresh();
                cx.notify();
            });
        });
        h_flex()
            .gap_2()
            .items_center()
            .child(bases)
            .children(self.xaero_map_picker(cx))
            .into_any_element()
    }

    /// The cursor's world position in the bottom-left corner. Clicking it
    /// swaps the read-out for X and Z fields with an explicit "Go" key.
    pub(super) fn coordinates(&self, cx: &mut Context<Self>) -> AnyElement {
        let form = self.form.as_ref().unwrap();
        if !form.jumping {
            // ia[plugin.world-explorer]: 打开坐标跳转 | 地图左下角 · 光标坐标读数 | 点读数换成 X、Z 输入框并聚焦 X；读数随鼠标在地图上移动
            return div()
                .id("map-cursor")
                .debug_selector(|| "map-cursor".into())
                .cursor_pointer()
                .font_family(theme::mono_font())
                .text_xs()
                .child(format!("X {:.0}  Z {:.0}", self.cursor[0], self.cursor[1]))
                .on_click(cx.listener(|this, _, window, cx| this.open_jump(window, cx)))
                .into_any_element();
        }
        let portal = form
            .jump_x
            .read(cx)
            .value()
            .trim()
            .parse::<f64>()
            .ok()
            .zip(form.jump_z.read(cx).value().trim().parse::<f64>().ok())
            .and_then(|(x, z)| {
                self.context.as_ref().and_then(|context| {
                    portal_coordinates(MapPoint { x, z }, &context.dimension).map(|coords| {
                        let target = if context.dimension == Dimension::Overworld {
                            Dimension::Nether
                        } else {
                            Dimension::Overworld
                        };
                        (coords, target)
                    })
                })
            });
        // ia[plugin.world-explorer]: 前往坐标 | 地图左下角 · X、Z 两个输入框与「前往」键 | 点「前往」或在任一框按 Enter 把视图中心移到该点并收回读数；Esc 取消；X 或 Z 无法读取时在框上说明，视图不动
        v_flex()
            .gap_1()
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    this.close_jump(cx);
                }
            }))
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .w(px(96.))
                            .debug_selector(|| "map-jump-x".into())
                            .child(Input::new(&form.jump_x).xsmall()),
                    )
                    .child(
                        div()
                            .w(px(96.))
                            .debug_selector(|| "map-jump-z".into())
                            .child(Input::new(&form.jump_z).xsmall()),
                    )
                    .child(
                        Key::new("map-go")
                            .label(tr!("map-go"))
                            .white()
                            .compact()
                            .debug_selector(|| "map-go".into())
                            .on_click(cx.listener(|this, _, _, cx| this.jump(cx))),
                    )
                    .child(
                        Key::new("map-jump-cancel")
                            .label(tr!("common-cancel"))
                            .white()
                            .compact()
                            .debug_selector(|| "map-jump-cancel".into())
                            .on_click(cx.listener(|this, _, _, cx| this.close_jump(cx))),
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
            .children(portal.map(|((x, z), target)| {
                // ia[plugin.world-explorer]: 输入坐标换算下界 | 地图左下角 · 坐标跳转输入下的换算行 | 输入 X、Z 后显示 8:1 坐标；可复制或跳到另一维度
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .child(tr!("map-portal-coordinates", x = x, z = z)),
                    )
                    .child(
                        Key::new("map-jump-copy-portal")
                            .label(tr!("map-copy-portal"))
                            .white()
                            .compact()
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(format!(
                                    "{x} {z}"
                                )));
                            })),
                    )
                    .child(
                        Key::new("map-jump-go-portal")
                            .label(tr!("map-go-portal"))
                            .white()
                            .compact()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(context) = &mut this.context {
                                    context.dimension = target.clone();
                                }
                                this.camera.x = x as f64;
                                this.camera.z = z as f64;
                                this.close_jump(cx);
                                this.reset_view();
                                this.context_changed();
                                this.refresh();
                                cx.notify();
                            })),
                    )
            }))
            .into_any_element()
    }

    pub(super) fn retry_button(&self, cx: &mut Context<Self>) -> Option<Key> {
        let count = self.failed.len() + self.objects.failed();
        // ia[plugin.world-explorer]: 重试失败瓦片 | 地图消息菜单 ·「重试失败的 N 块」 | 一次重新派发全部失败块；没有失败时隐藏；暂停的来源重启后恢复
        (count > 0).then(|| {
            Key::new("map-retry")
                .label(tr!("map-retry-failed", count = count))
                .white()
                .compact()
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
