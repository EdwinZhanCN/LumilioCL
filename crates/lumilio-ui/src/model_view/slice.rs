//! Slicing and selecting in the studio's viewport (plan schematic-studio,
//! T25): 切片's dropdown draws one layer and those below it along X, Y or
//! Z, and the layer field over it (or `[` and `]`) moves through the
//! layers; a drag with Ctrl (or ⌘) from one block to another selects the
//! box between them. Both are the picture's, kept here; the studio reads
//! them for the materials and as a replacement's scope.

use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Entity, Focusable as _, Pixels, Point, Subscription, Window, div, px,
};
use gpui_component::input::{InputEvent, InputState, NumberInput, NumberInputEvent, StepAction};
use gpui_component::select::{SearchableVec, SelectEvent, SelectState};
use gpui_component::{IndexPath, Sizable as _, h_flex};
use lumilio_schematic_render::{Slice, SliceAxis};

use super::Viewer;
use super::worker::Job;
use crate::theme::ShellColors;
use crate::{kit, tr};

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
        if let Some(slice) = self.slice {
            self.go_to_layer(slice.layer + by, cx);
        }
    }

    /// Slices at `layer`, within the schematic, along the axis in use.
    pub(crate) fn go_to_layer(&mut self, layer: i32, cx: &mut Context<Self>) {
        let Some(slice) = self.slice else {
            return;
        };
        let Some((bottom, top)) = self.layers(slice.axis) else {
            return;
        };
        let layer = layer.clamp(bottom, top);
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

    /// Chooses 切片's `choice`, as its dropdown lists them: 0 is 不切片,
    /// then X, Y and Z. An axis slices along it, at the same layer when it
    /// is in range, else from the top.
    pub(crate) fn choose_slice(&mut self, choice: usize, cx: &mut Context<Self>) {
        match choice.checked_sub(1).and_then(|at| SliceAxis::ALL.get(at)) {
            Some(axis) => self.slice_along(*axis, cx),
            None => self.set_slice(None, cx),
        }
    }

    /// 切片's choice in use: 0 when not slicing, else 1 + the axis.
    fn slice_choice(&self) -> usize {
        self.slice.map_or(0, |slice| slice.axis.index() + 1)
    }

    /// Asks for 切片's controls once: an input state installs focus
    /// observers, so they are made after this draw, with its window.
    pub(super) fn want_slice_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.inline || self.slice_controls_asked {
            return;
        }
        self.slice_controls_asked = true;
        let viewer = cx.weak_entity();
        window.defer(cx, move |window, cx| {
            let _ = viewer.update(cx, |viewer, cx| {
                viewer.slice_controls = Some(SliceControls::new(viewer.slice_choice(), window, cx));
                cx.notify();
            });
        });
    }

    /// Keeps 切片's dropdown and the layer field showing the slice, however
    /// it changed (`[` and `]`, the field's own steps, another axis). The
    /// field is left alone while it is being typed into.
    pub(super) fn sync_slice_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (choice, slice) = (self.slice_choice(), self.slice);
        let Some(controls) = self.slice_controls.as_mut() else {
            return;
        };
        if controls.language != crate::i18n::generation() {
            controls.language = crate::i18n::generation();
            controls.pick.update(cx, |pick, cx| {
                pick.set_items(SearchableVec::new(slice_choices()), window, cx);
                pick.set_selected_index(Some(IndexPath::new(choice)), window, cx);
            });
        }
        if controls.pick.read(cx).selected_index(cx) != Some(IndexPath::new(choice)) {
            controls.pick.update(cx, |pick, cx| {
                pick.set_selected_index(Some(IndexPath::new(choice)), window, cx);
            });
        }
        let Some(slice) = slice else {
            return;
        };
        let text = slice.layer.to_string();
        let typing = controls.layer.read(cx).focus_handle(cx).is_focused(window);
        if (!typing || std::mem::take(&mut controls.resync))
            && controls.layer.read(cx).value() != text.as_str()
        {
            controls
                .layer
                .update(cx, |field, cx| field.set_value(text, window, cx));
        }
    }

    /// The trailing plate's 切片: a label and the dropdown 不切片 / X 轴 /
    /// Y 轴 / Z 轴, as the discover page's 排序方式. `None` until made.
    pub(super) fn slice_select(&self, colors: ShellColors) -> Option<AnyElement> {
        let controls = self.slice_controls.as_ref()?;
        // ia[plugin.litematica]: 切片 / 换切片的轴 | 工作室视口右下角「切片」下拉：不切片、X 轴、Y 轴、Z 轴，框里写着正在用的 | 只画当前层及以下，切口露出这一层的顶面；第一次从最高层开始，换轴时层在范围内就保留，否则从最高层开始；点不到被切掉的层；选不切片画回整份投影 | 每沿一个轴第一次切片时重新网格化一次，之后逐层只换一个 uniform（T25）
        Some(
            kit::labeled_select(tr!("model-slice"), &controls.pick, SLICE_SELECT, colors)
                .debug_selector(|| "model-slice".into())
                .into_any_element(),
        )
    }

    /// While slicing: the layer, a number field with − and +.
    pub(super) fn layer_field(&self, colors: ShellColors) -> Option<AnyElement> {
        let controls = self.slice_controls.as_ref()?;
        self.slice?;
        // ia[plugin.litematica]: 逐层看切片 | 切片时右下角「切片」上方的层数框：− 与 +、直接输入层号，视口聚焦时也可按 [ 与 ] | 往下或往上一层；输入的层号超出投影时停在第一层或最后一层，离开输入框后框里写回实际的层 | 这一层有多少方块写在材料里（T25）
        Some(
            h_flex()
                .debug_selector(|| "model-slice-layer".into())
                .gap_2()
                .items_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.muted)
                        .child(tr!("model-slice-layer-label")),
                )
                .child(
                    div()
                        .w(px(LAYER_FIELD))
                        .child(NumberInput::new(&controls.layer).small()),
                )
                .into_any_element(),
        )
    }
}

/// 切片's dropdown, as wide as its longest choice.
const SLICE_SELECT: f32 = 96.;

/// The layer field: room for a three-digit layer between − and +.
const LAYER_FIELD: f32 = 96.;

/// 切片's choices, in order: 不切片, then an axis each.
fn slice_choices() -> Vec<String> {
    std::iter::once(tr!("model-slice-off").to_owned())
        .chain(
            SliceAxis::ALL
                .into_iter()
                .map(|axis| tr!("model-slice-along", axis = axis_name(axis))),
        )
        .collect()
}

/// 切片's dropdown and the layer field, made on a draw that has a window.
pub(super) struct SliceControls {
    pick: Entity<SelectState<SearchableVec<String>>>,
    layer: Entity<InputState>,
    /// The language the dropdown's words were written in: kept in its
    /// state, they are written again after a switch.
    language: u32,
    /// The field shows the slice again though it has focus: a step, Enter
    /// or leaving it.
    resync: bool,
    _subscriptions: Vec<Subscription>,
}

impl SliceControls {
    fn new(choice: usize, window: &mut Window, cx: &mut Context<Viewer>) -> Self {
        let pick = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(slice_choices()),
                Some(IndexPath::new(choice)),
                window,
                cx,
            )
        });
        let layer = cx.new(|cx| {
            InputState::new(window, cx)
                .validate(|text, _| text.is_empty() || text == "-" || text.parse::<i32>().is_ok())
        });
        let subscriptions = vec![
            cx.subscribe_in(
                &pick,
                window,
                |viewer, pick, event: &SelectEvent<SearchableVec<String>>, _, cx| {
                    if let SelectEvent::Confirm(Some(_)) = event
                        && let Some(choice) = pick.read(cx).selected_index(cx)
                    {
                        viewer.choose_slice(choice.row, cx);
                    }
                },
            ),
            cx.subscribe_in(
                &layer,
                window,
                |viewer, _, event: &NumberInputEvent, _, cx| {
                    let NumberInputEvent::Step(action) = event;
                    viewer.step_layer(
                        match action {
                            StepAction::Increment => 1,
                            StepAction::Decrement => -1,
                        },
                        cx,
                    );
                    viewer.resync_layer(cx);
                },
            ),
            cx.subscribe_in(
                &layer,
                window,
                |viewer, field, event: &InputEvent, _, cx| match event {
                    InputEvent::Change => {
                        if let Ok(layer) = field.read(cx).value().trim().parse::<i32>() {
                            viewer.go_to_layer(layer, cx);
                        }
                    }
                    InputEvent::PressEnter { .. } | InputEvent::Blur => viewer.resync_layer(cx),
                    _ => {}
                },
            ),
        ];
        Self {
            pick,
            layer,
            language: crate::i18n::generation(),
            resync: false,
            _subscriptions: subscriptions,
        }
    }
}

impl Viewer {
    /// Shows the slice's layer in the field again on the next draw, though
    /// the field has focus.
    fn resync_layer(&mut self, cx: &mut Context<Self>) {
        if let Some(controls) = self.slice_controls.as_mut() {
            controls.resync = true;
        }
        cx.notify();
    }

    /// The layer field, for tests.
    #[cfg(test)]
    pub(crate) fn layer_field_for_test(&self) -> Option<Entity<InputState>> {
        self.slice_controls
            .as_ref()
            .map(|controls| controls.layer.clone())
    }

    /// 切片's dropdown, for tests.
    #[cfg(test)]
    pub(crate) fn slice_pick_for_test(&self) -> Option<Entity<SelectState<SearchableVec<String>>>> {
        self.slice_controls
            .as_ref()
            .map(|controls| controls.pick.clone())
    }
}

/// An axis as the strip writes it.
fn axis_name(axis: SliceAxis) -> &'static str {
    ["X", "Y", "Z"][axis.index()]
}
