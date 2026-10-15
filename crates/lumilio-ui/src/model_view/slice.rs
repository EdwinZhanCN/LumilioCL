//! Slicing and selecting in the studio's viewport (plan schematic-studio,
//! T25): the tool strip's 切片 draws one layer and those below it along X, Y
//! or Z, stepped with its keys or `[` and `]`; a drag with Ctrl (or ⌘) from
//! one block to another selects the box between them. Both are the
//! picture's, kept here; the studio reads them for the materials and as a
//! replacement's scope.

use gpui::prelude::*;
use gpui::{AnyElement, Context, Pixels, Point, div};
use gpui_component::{Selectable as _, Sizable as _, h_flex};
use lumilio_schematic_render::{Slice, SliceAxis};

use super::worker::Job;
use super::{State, Viewer};
use crate::controls::Segments;
use crate::key::Key;
use crate::theme::{self, ShellColors};
use crate::tr;

/// A box of cells, its first and last corners both included.
pub(crate) type Cells = ([i32; 3], [i32; 3]);

/// A Ctrl-drag in progress: where it started, and its block once picked.
pub(super) struct SelectDrag {
    pub(super) from: Option<[i32; 3]>,
}

impl Viewer {
    /// The slice the picture draws, if any.
    pub(crate) fn slice(&self) -> Option<Slice> {
        self.slice
    }

    /// The selected box of cells, both corners included, if any.
    pub(crate) fn selection(&self) -> Option<Cells> {
        self.selection
    }

    /// The cells of the slice's layer, across the whole schematic (T24: a
    /// replacement's scope).
    pub(crate) fn slice_cells(&self) -> Option<Cells> {
        let (slice, (mut min, mut max)) = (self.slice?, self.cells?);
        let axis = slice.axis.index();
        (min[axis], max[axis]) = (slice.layer, slice.layer);
        Some((min, max))
    }

    /// The blocks in the slice's layer, by id, most first; empty until the
    /// worker answers.
    pub(crate) fn layer_blocks(&self) -> &[(String, u64)] {
        &self.layer_blocks
    }

    /// The blocks in the selection, by id, most first; empty until the
    /// worker answers.
    pub(crate) fn selection_blocks(&self) -> &[(String, u64)] {
        &self.selection_blocks
    }

    /// The layers along `axis` the schematic has, once it is loaded.
    fn layers(&self, axis: SliceAxis) -> Option<(i32, i32)> {
        let (min, max) = self.cells?;
        Some((min[axis.index()], max[axis.index()]))
    }

    /// Slices along the last axis used, at its top layer, or stops slicing.
    pub(crate) fn toggle_slice(&mut self, cx: &mut Context<Self>) {
        let slice = match self.slice {
            Some(_) => None,
            None => self.layers(self.slice_axis).map(|(_, top)| Slice {
                axis: self.slice_axis,
                layer: top,
            }),
        };
        self.set_slice(slice, cx);
    }

    /// Slices along `axis`, at the same layer when it is in range, else its
    /// top.
    pub(crate) fn slice_along(&mut self, axis: SliceAxis, cx: &mut Context<Self>) {
        self.slice_axis = axis;
        let Some((bottom, top)) = self.layers(axis) else {
            return;
        };
        let layer = self
            .slice
            .filter(|slice| slice.axis == axis)
            .map_or(top, |slice| slice.layer.clamp(bottom, top));
        self.set_slice(Some(Slice { axis, layer }), cx);
    }

    /// Moves the slice `by` layers, within the schematic.
    pub(crate) fn step_layer(&mut self, by: i32, cx: &mut Context<Self>) {
        let Some(slice) = self.slice else {
            return;
        };
        let Some((bottom, top)) = self.layers(slice.axis) else {
            return;
        };
        let layer = (slice.layer + by).clamp(bottom, top);
        if layer != slice.layer {
            self.set_slice(Some(Slice { layer, ..slice }), cx);
        }
    }

    pub(crate) fn set_slice(&mut self, slice: Option<Slice>, cx: &mut Context<Self>) {
        if self.slice == slice {
            return;
        }
        self.slice = slice;
        self.layer_blocks.clear();
        self.send_slice(cx);
        self.tell(cx);
    }

    /// Tells the worker the slice, and keeps what it says the layer holds.
    pub(super) fn send_slice(&mut self, cx: &mut Context<Self>) {
        let Some(worker) = &self.worker else {
            return;
        };
        let (reply, answer) = async_channel::bounded(1);
        worker.mailbox.queue(Job::Slice {
            slice: self.slice,
            reply,
        });
        let asked = self.slice;
        self.slice_task = Some(cx.spawn(async move |this, cx| {
            let Ok(Ok(blocks)) = answer.recv().await else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                if this.slice == asked {
                    this.layer_blocks = blocks;
                    this.tell(cx);
                }
            });
        }));
    }

    /// Selects the box between two cells, or nothing.
    pub(crate) fn set_selection(&mut self, cells: Option<Cells>, cx: &mut Context<Self>) {
        let cells = cells.map(|(a, b)| {
            (
                [0, 1, 2].map(|axis| a[axis].min(b[axis])),
                [0, 1, 2].map(|axis| a[axis].max(b[axis])),
            )
        });
        if self.selection == cells {
            return;
        }
        self.selection = cells;
        self.selection_blocks.clear();
        self.sync_selection();
        if let Some((min, max)) = cells
            && let Some(answer) = self.blocks_in(min, max)
        {
            self.selection_task = Some(cx.spawn(async move |this, cx| {
                let Ok(blocks) = answer.recv().await else {
                    return;
                };
                let _ = this.update(cx, |this, cx| {
                    if this.selection == cells {
                        this.selection_blocks = blocks;
                        this.tell(cx);
                    }
                });
            }));
        }
        self.tell(cx);
    }

    /// Tells the worker the selection in the theme's colour, on a change.
    pub(super) fn sync_selection(&mut self) {
        let wanted = self.selection.map(|cells| (cells, self.selection_color));
        if wanted == self.selection_sent {
            return;
        }
        if let Some(worker) = &self.worker {
            worker.mailbox.queue(Job::Selection {
                cells: self.selection,
                color: self.selection_color,
            });
            self.selection_sent = wanted;
        }
    }

    /// The blocks in a box of cells, asked of the worker; `None` before the
    /// scene is loading.
    pub(crate) fn blocks_in(
        &self,
        min: [i32; 3],
        max: [i32; 3],
    ) -> Option<async_channel::Receiver<Vec<(String, u64)>>> {
        let worker = self.worker.as_ref()?;
        let (reply, answer) = async_channel::bounded(1);
        worker.mailbox.queue(Job::BlocksIn { min, max, reply });
        Some(answer)
    }

    /// Selects a box whose blocks are already counted, as a Ctrl-drag and
    /// the worker's answer would.
    #[cfg(test)]
    pub(crate) fn selected_for_test(
        &mut self,
        cells: Cells,
        blocks: Vec<(String, u64)>,
        cx: &mut Context<Self>,
    ) {
        self.selection = Some(cells);
        self.selection_blocks = blocks;
        self.tell(cx);
    }

    /// A Ctrl-drag starts at `position`: its first block is picked.
    pub(super) fn start_selecting(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.selecting = Some(SelectDrag { from: None });
        let Some(at) = self.fraction(position) else {
            return;
        };
        let Some(answer) = self.ask_pick(at) else {
            return;
        };
        self.select_task = Some(cx.spawn(async move |this, cx| {
            let picked = answer.recv().await.ok().flatten();
            let _ = this.update(cx, |this, _| {
                if let Some(drag) = &mut this.selecting {
                    drag.from = picked.map(|picked| picked.position);
                }
            });
        }));
    }

    /// The drag ends at `position`: the box runs from its first block to
    /// the one there. A drag that started or ended on nothing selects
    /// nothing.
    pub(super) fn finish_selecting(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(drag) = self.selecting.take() else {
            return;
        };
        let (Some(at), Some(from)) = (self.fraction(position), drag.from) else {
            return;
        };
        let Some(answer) = self.ask_pick(at) else {
            return;
        };
        self.select_task = Some(cx.spawn(async move |this, cx| {
            let Some(to) = answer.recv().await.ok().flatten() else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.set_selection(Some((from, to.position)), cx);
            });
        }));
    }

    /// The tool strip's slice controls: 切片, and while slicing the axis,
    /// the layer keys and what the layer holds.
    pub(super) fn slice_controls(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        let ready = self.state == State::Ready && self.cells.is_some();
        let viewer = cx.weak_entity();
        // ia[plugin.litematica]: 切片 | 工作室视口工具条「切片」（再点关闭） | 只画当前层及以下，切口露出这一层的顶面；默认从最高层开始；点不到被切掉的层 | 第一次沿某个轴切片时重新网格化一次，之后逐层只换一个 uniform（T25）
        let toggle = Key::new("model-slice")
            .label(tr!("model-slice"))
            .white()
            .small()
            .selected(self.slice.is_some())
            .disabled(!ready)
            .debug_selector(|| "model-slice".into())
            .on_click(move |_, _, cx| {
                let _ = viewer.update(cx, |viewer, cx| viewer.toggle_slice(cx));
            });
        let Some(slice) = self.slice else {
            return toggle.into_any_element();
        };
        let viewer = cx.weak_entity();
        // ia[plugin.litematica]: 换切片的轴 | 切片中工具条的 X / Y / Z 分段 | 沿新轴切，层在范围内就保留，否则从最高层开始 | 每换一次轴重新网格化一次（T25）
        let axes = Segments::new(
            "model-slice-axis",
            &["X", "Y", "Z"],
            slice.axis.index(),
            move |index, _, cx| {
                let _ = viewer.update(cx, |viewer, cx| {
                    viewer.slice_along(SliceAxis::ALL[index], cx);
                });
            },
        );
        let step = |id: &'static str, label: &'static str, by: i32, tip: &'static str| {
            let viewer = cx.weak_entity();
            Key::new(id)
                .label(label)
                .ghost()
                .small()
                .tooltip(tip)
                .debug_selector(move || id.into())
                .on_click(move |_, _, cx| {
                    let _ = viewer.update(cx, |viewer, cx| viewer.step_layer(by, cx));
                })
        };
        let count: u64 = self.layer_blocks.iter().map(|(_, count)| count).sum();
        let axis = ["X", "Y", "Z"][slice.axis.index()];
        // ia[plugin.litematica]: 逐层看切片 | 切片中工具条的 ‹ › 键，视口聚焦时按 [ 与 ] | 往下或往上一层，读数写「轴 层号 · 这一层的方块数」 | 停在投影的第一层与最后一层（T25）
        h_flex()
            .gap_1()
            .items_center()
            .child(toggle)
            .child(axes)
            .child(step("model-slice-down", "‹", -1, tr!("model-slice-down")))
            .child(
                div()
                    .debug_selector(|| "model-slice-layer".into())
                    .px_1()
                    .font_family(theme::mono_font())
                    .text_xs()
                    .text_color(colors.foreground)
                    .child(tr!(
                        "model-slice-layer",
                        axis = axis,
                        layer = slice.layer,
                        count = count
                    )),
            )
            .child(step("model-slice-up", "›", 1, tr!("model-slice-up")))
            .into_any_element()
    }
}
