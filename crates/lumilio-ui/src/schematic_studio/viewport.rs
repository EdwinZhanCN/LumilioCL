//! The workbench's picture (D7, D22, D27): the resident 3D viewport filling
//! the leading column, its controls floating on its bottom edge, and above
//! them a read-out plate that names what the pointer (or, flying, the sight
//! line) is on: the block's icon, name, property words and position. With
//! nothing under the pointer the plate gives the controls. A click on a
//! block inspects that one block: the plate gives way to its inspect card
//! (T32, D31); the materials keep the kind they chose.

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, div, px};
use gpui_component::{h_flex, v_flex};

use super::words::property_words;
use super::{LoadModel, StudioView, muted_line};
use crate::model_view::Viewer;
use crate::theme::ShellColors;
use crate::{theme, tr};

/// The plate's icon side.
const PLATE_ICON: f32 = 24.;

/// A viewer for `file`. The previous one is dropped by the caller, and with
/// it its worker and frames, as closing the old preview modal did.
pub(super) fn viewer(file: &str, load: LoadModel, cx: &mut Context<StudioView>) -> Entity<Viewer> {
    let file = file.to_owned();
    let studio = cx.weak_entity();
    // What the pointer is on and the inspected block are read from the
    // viewer each time the studio draws. The viewer tells mid-update, at
    // times inside an update of the studio's (its card's close key), so the
    // studio acts once those are over: it asks for the pictures of the
    // inspected block's items and draws again.
    let on_pointed = std::rc::Rc::new(move |cx: &mut App| {
        let studio = studio.clone();
        cx.defer(move |cx| {
            let _ = studio.update(cx, |studio, cx| {
                studio.want_item_icons(cx);
                cx.notify();
            });
        });
    });
    cx.new(|cx| {
        Viewer::inline(
            std::rc::Rc::new(move |request, window, cx| load(file.clone(), request, window, cx)),
            on_pointed,
            cx,
        )
    })
}

/// Splits `minecraft:oak_stairs[facing=east,half=top]` into the id and its
/// properties in file order; a bare id has none.
pub(crate) fn split_state(state: &str) -> (String, Vec<(String, String)>) {
    let Some((id, rest)) = state.split_once('[') else {
        return (state.trim().to_owned(), Vec::new());
    };
    let properties = rest
        .trim_end_matches(']')
        .split(',')
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            Some((key.trim().to_owned(), value.trim().to_owned()))
        })
        .collect();
    (id.trim().to_owned(), properties)
}

impl StudioView {
    pub(super) fn viewport_column(
        &self,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let content = match &self.viewer {
            // ia[plugin.litematica]: 观察投影 | 工作室窗口左栏常驻 3D 视口（左下角浮着 Orbital / Explore 与「角度」，右下角浮着「切片」下拉） | Orbital 拖拽旋转、滚轮缩放；Explore 点击捕获鼠标后 WASD 飞行；画不出的方块画成紫黑缺失贴图的整块占位，仍可点选 | 视口常驻在工作室窗口里，不是弹窗也不在游戏页内（D7、D22、D27）
            Some(viewer) => viewer.clone().into_any_element(),
            None => v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .rounded(px(6.))
                .bg(colors.surface)
                .child(muted_line(tr!("studio-viewport-empty"), colors))
                .into_any_element(),
        };
        v_flex()
            .debug_selector(|| "studio-viewport".into())
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .min_h_0()
            .child(content)
            .children(self.read_out(colors, cx))
            .into_any_element()
    }

    /// What the plate says about `state`: its id (for the icon), the game's
    /// name for it, and its property words. Never the bare ID when a name
    /// is known (D27).
    pub(super) fn read_out_words(&self, state: &str) -> (String, String, Vec<String>) {
        let (id, properties) = split_state(state);
        let name = self.name_of(&id);
        (id, name, property_words(&properties))
    }

    /// The plate above the tool strip, or the inspected block's card in its
    /// place; `None` until the picture is drawn.
    fn read_out(&self, colors: ShellColors, cx: &mut Context<Self>) -> Option<AnyElement> {
        let viewer = self.viewer.as_ref()?.read(cx);
        if !viewer.is_drawn() {
            return None;
        }
        if let Some(inspected) = viewer.inspected().cloned() {
            return Some(self.inspect_card(&inspected, colors, cx));
        }
        let viewer = self.viewer.as_ref()?.read(cx);
        let clearance = viewer.tool_clearance();
        let body = match viewer.pointed() {
            Some(pointed) => {
                let (id, name, words) = self.read_out_words(&pointed.state);
                let [x, y, z] = pointed.position;
                h_flex()
                    .debug_selector(|| "studio-read-out-block".into())
                    .gap_2()
                    .items_center()
                    .child(self.icon(&id, PLATE_ICON, colors))
                    .child(
                        div()
                            .debug_selector(|| "studio-read-out-name".into())
                            .flex_none()
                            .text_sm()
                            .text_color(colors.foreground)
                            .child(name),
                    )
                    .children((!words.is_empty()).then(|| {
                        div()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .text_color(colors.muted)
                            .child(words.join(tr!("common-list-separator")))
                    }))
                    .child(
                        div()
                            .flex_none()
                            .font_family(theme::mono_font())
                            .text_xs()
                            .text_color(colors.muted)
                            .child(format!("{x}, {y}, {z}")),
                    )
                    .into_any_element()
            }
            None => div()
                .debug_selector(|| "studio-read-out-hint".into())
                .text_xs()
                .text_color(colors.muted)
                .child(viewer.hint())
                .into_any_element(),
        };
        // ia[plugin.litematica]: 读数板 | 工作室视口左下、Orbital / Explore 上方 | 指着或飞行中看着的方块写图标、名字、属性与坐标；什么都没指时写当前模式的操作提示；检视一个方块时让位给检视卡；切片时右下角多一行，读数板与角落的检视卡一起抬高让开 | 原来视口下方的提示行与右上的方块卡都并进这里（D27、T32）
        Some(
            div()
                .debug_selector(|| "studio-read-out".into())
                .absolute()
                .left_3()
                .bottom(px(clearance))
                .max_w(gpui::relative(0.92))
                .px_2()
                .py_1()
                .rounded(px(4.))
                .bg(colors.body.panel.opacity(0.92))
                .child(body)
                .into_any_element(),
        )
    }
}
