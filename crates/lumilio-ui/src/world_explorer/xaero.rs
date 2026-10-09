//! Linking a save to its Xaero Minimap directory. A same-named directory is
//! only suggested; the person confirms it here, and the link is saved.
use super::{Command, MapView};
use crate::{key::Key, tr};
use gpui::{AnyElement, Context, div, prelude::*};
use gpui_component::{Sizable as _, h_flex};
use lumilio_plugin_api::map::WorldId;

impl MapView {
    /// The save on screen and the directory suggested for it, if the person
    /// has not linked one yet.
    fn xaero_suggestion(&self) -> Option<(String, String)> {
        let context = self.context.as_ref()?;
        let WorldId::Save { folder, .. } = &context.world else {
            return None;
        };
        let world = self
            .contexts
            .iter()
            .find(|world| &world.context.world == &context.world)?;
        Some((folder.clone(), world.suggested_xaero.clone()?))
    }

    /// A line offering to link the suggested directory, with the key that does.
    pub(super) fn xaero_link_prompt(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (folder, dir) = self.xaero_suggestion()?;
        // ia[plugin.world-explorer]: 关联 Xaero 路径点 | 地图左下角提示 · 「发现同名的 Xaero 小地图数据」与「关联」 | 只在存档有同名目录且未关联时出现；确认后保存，路径点图层随即可用；不会自动关联
        Some(
            h_flex()
                .gap_2()
                .items_center()
                .debug_selector(|| "map-xaero-link".into())
                .child(div().child(tr!("map-xaero-suggestion", dir = dir.clone())))
                .child(
                    Key::new("map-xaero-link-key")
                        .label(tr!("map-xaero-link"))
                        .white()
                        .small()
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
