//! Linking a save to its Xaero Minimap directory. A same-named directory is
//! only suggested; the person confirms it here, and the link is saved.
use super::{Command, MapView};
use crate::{key::Key, tr};
use gpui::{Anchor, AnyElement, Context, div, prelude::*, px};
use gpui_component::{Sizable as _, popover::Popover, v_flex};
use lumilio_plugin_api::map::{Dimension, SourceLink, WorldId};

impl MapView {
    pub(super) fn xaero_map_ids(&self) -> Vec<String> {
        let Some(context) = self.context.as_ref() else {
            return vec![];
        };
        let dimension = match context.dimension {
            Dimension::Overworld => "null",
            Dimension::Nether => "DIM-1",
            Dimension::End => "DIM1",
            Dimension::Custom(_) => return vec![],
        };
        context
            .sources
            .iter()
            .filter_map(|source| {
                let SourceLink::XaeroWorldMap(path) = source else {
                    return None;
                };
                let mut parts = path.split('/');
                let (Some(_world), Some(found), Some(id), None) =
                    (parts.next(), parts.next(), parts.next(), parts.next())
                else {
                    return None;
                };
                (found == dimension && id.starts_with("mw$")).then(|| path.clone())
            })
            .collect()
    }

    pub(super) fn xaero_map_choice(&self) -> Option<String> {
        let context = self.context.as_ref()?;
        let ids = self.xaero_map_ids();
        if ids.len() == 1 {
            return ids.into_iter().next();
        }
        self.xaero_maps
            .get(&(context.world.clone(), context.dimension.clone()))
            .filter(|choice| ids.contains(choice))
            .cloned()
    }

    pub(super) fn xaero_map_picker(&self, cx: &mut Context<Self>) -> Option<Popover> {
        if self.base_kind() != Some("map-base-xaero") {
            return None;
        }
        let ids = self.xaero_map_ids();
        if ids.len() < 2 {
            return None;
        }
        let chosen = self.xaero_map_choice();
        let label = chosen
            .as_deref()
            .and_then(|path| path.rsplit('/').next())
            .map(str::to_owned)
            .unwrap_or_else(|| tr!("map-xaero-choose").to_string());
        let target = cx.weak_entity();
        // ia[plugin.world-explorer]: 选择 Xaero 世界地图 | 地图右上角 · Xaero 底图旁的地图下拉 | 同维度多份 mw$ 地图时选择其中一份；切换后保留中心、缩放与图层，瓦片缓存按选择隔离
        Some(
            Popover::new("map-xaero-map-picker")
                .anchor(Anchor::TopRight)
                .trigger(
                    Key::new("map-xaero-map")
                        .label(label)
                        .white()
                        .small()
                        .debug_selector(|| "map-xaero-map".into()),
                )
                .content(move |_, _, _| {
                    v_flex()
                        .w(px(180.))
                        .gap_1()
                        .children(ids.iter().enumerate().map(|(index, path)| {
                            let picked = path.clone();
                            let target = target.clone();
                            Key::new(format!("map-xaero-map-option-{index}"))
                                .label(path.rsplit('/').next().unwrap_or(path).to_owned())
                                .white()
                                .small()
                                .debug_selector(move || format!("map-xaero-map-option-{index}"))
                                .on_click(move |_, _, cx| {
                                    let _ = target.update(cx, |this, cx| {
                                        if let Some(context) = &this.context {
                                            this.xaero_maps.insert(
                                                (context.world.clone(), context.dimension.clone()),
                                                picked.clone(),
                                            );
                                        }
                                        this.reset_view();
                                        this.refresh();
                                        cx.notify();
                                    });
                                })
                        }))
                        .into_any_element()
                }),
        )
    }

    /// The save on screen and the directory suggested for it, if the person
    /// has not linked one yet.
    pub(super) fn xaero_suggestion(&self) -> Option<(String, String)> {
        let context = self.context.as_ref()?;
        let WorldId::Save { folder, .. } = &context.world else {
            return None;
        };
        let world = self
            .contexts
            .iter()
            .find(|world| world.context.world == context.world)?;
        Some((folder.clone(), world.suggested_xaero.clone()?))
    }

    /// A line offering to link the suggested directory, with the key that does.
    pub(super) fn xaero_link_prompt(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (folder, dir) = self.xaero_suggestion()?;
        // ia[plugin.world-explorer]: 关联 Xaero 路径点 | 地图消息菜单 · 「发现同名的 Xaero 小地图数据」与「关联」 | 只在存档有同名目录且未关联时出现；确认后保存，路径点图层随即可用；不会自动关联
        Some(
            v_flex()
                .gap_2()
                .items_start()
                .debug_selector(|| "map-xaero-link".into())
                .child(div().child(tr!("map-xaero-suggestion", dir = dir.clone())))
                .child(
                    Key::new("map-xaero-link-key")
                        .label(tr!("map-xaero-link"))
                        .white()
                        .compact()
                        .debug_selector(|| "map-xaero-link-key".into())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(connection) = &this.connection {
                                let _ = connection.send.try_send(Command::LinkXaero {
                                    folder: folder.clone(),
                                    dir: dir.clone(),
                                });
                            }
                            cx.notify();
                        })),
                )
                .into_any_element(),
        )
    }
}
