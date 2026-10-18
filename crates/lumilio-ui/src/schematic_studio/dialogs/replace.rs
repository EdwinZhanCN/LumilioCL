//! 替换方块… (plan schematic-studio, T13, T24): a table of rows, 从 → 到.
//! Each row's source is one of the file's blocks with a chip per property
//! (a value, or 任意; 不限属性 drops them all); its target is chosen from
//! the file's game version's blocks, recent ones first, and keeps every
//! property it shares with the block it replaces. The scope is all,
//! one region, the current slice or the current selection. What would
//! change is counted on the background executor whenever anything changes,
//! row by row, and the commit key says the total. A table can be kept as a
//! named 方案 in launcher data and applied to another schematic; both are
//! on the footer's leading side, as they act on the whole table.
//!
//! Every choice in the table is a dropdown: a row repeats, and segment keys
//! (their LED band, their width that follows the labels) would make each
//! row taller and the columns ragged. The scope's options include region
//! names, which are the file's data, so it is a dropdown too.

use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    Anchor, AnyElement, App, Context, Entity, Focusable as _, SharedString, Subscription, Task,
    WeakEntity, Window, div, px,
};
use gpui_component::IndexPath;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::popover::Popover;
use gpui_component::select::{SearchableVec, Select, SelectEvent, SelectItem, SelectState};
use gpui_component::{Sizable as _, h_flex, v_flex};
use lumilio_core::{BlockCatalog, BlockNames, ReplaceSchemes, SchematicOperation, SchemeRow};
use lumilio_schematic::UniversalSchematic;
use lumilio_schematic::replace::{Mapping, MappingRow, PropertyMatch, Scope, StateMatcher};
use lumilio_schematic::stats::BlockCount;

use super::{DialogForm, Submission, field, note};
use crate::controls::Fader;
use crate::key::{Key, KeySize};
use crate::kit::{self, MenuEntry};
use crate::model_view::Cells;
use crate::schematic_studio::words::block_name;
use crate::schematic_studio::writes::Edited;
use crate::schematic_studio::{Loaded, StudioView, tree};
use crate::theme::{self, ShellColors};
use crate::tr;

/// A property chip's dropdown is searchable past this many values.
const SEARCH_PAST: usize = 8;

/// A property chip's dropdown: room for values such as `ascending_east`.
const CHIP_WIDTH: f32 = 140.;

/// A block picker's list: wider than half the dialog, so a name and its ID
/// both fit.
const BLOCK_MENU: f32 = 360.;

type Dropdown = Entity<SelectState<SearchableVec<String>>>;

/// The scope's dropdown.
type ScopeDropdown = Entity<SelectState<SearchableVec<ScopeChoice>>>;

/// A row's source or target picker: blocks by name, chosen by ID.
type BlockDropdown = Entity<SelectState<SearchableVec<BlockChoice>>>;

/// A block in a picker: its name in the launcher's language as the label,
/// its ID as the value and a mono caption (D28: an ID is the label only
/// when no name is known). The search finds either.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BlockChoice {
    id: String,
    name: SharedString,
}

impl BlockChoice {
    pub(crate) fn new(names: &BlockNames, id: &str) -> Self {
        Self {
            id: id.to_owned(),
            name: block_name(names, id).into(),
        }
    }

    fn all(names: &BlockNames, ids: &[String]) -> Vec<Self> {
        ids.iter().map(|id| Self::new(names, id)).collect()
    }
}

impl SelectItem for BlockChoice {
    type Value = String;

    fn title(&self) -> SharedString {
        self.name.clone()
    }

    /// The name first and whole; the ID after it gives way when the two do
    /// not fit (D28: the name is the label, the ID a caption).
    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = ShellColors::current(cx);
        h_flex()
            .w_full()
            .min_w_0()
            .gap_3()
            .child(
                div()
                    .flex_none()
                    .max_w(gpui::relative(0.7))
                    .truncate()
                    .child(self.name.clone()),
            )
            .when(self.name.as_ref() != self.id, |row| {
                row.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_family(theme::mono_font())
                        .text_xs()
                        .text_color(colors.muted)
                        .child(self.id.clone()),
                )
            })
    }

    fn value(&self) -> &String {
        &self.id
    }

    fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();
        self.name.to_lowercase().contains(&query) || self.id.to_lowercase().contains(&query)
    }
}

/// One scope in its dropdown, by its place in the dialog's list: two
/// regions may share a name.
#[derive(Clone, Debug, PartialEq)]
struct ScopeChoice {
    index: usize,
    label: SharedString,
}

impl SelectItem for ScopeChoice {
    type Value = usize;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &usize {
        &self.index
    }
}

/// One property's choice: 任意 (index 0) or one of the values the file
/// has, in a dropdown.
struct Pick {
    labels: Vec<String>,
    chosen: usize,
    dropdown: Dropdown,
}

/// One property chip of a row's source: index 0 is 任意.
struct Chip {
    name: String,
    pick: Pick,
}

/// One row of the table.
struct Row {
    source: BlockDropdown,
    /// 不限属性: every state of the source block.
    any: bool,
    chips: Vec<Chip>,
    target: BlockDropdown,
    /// A target proposed for this row (T37), chosen once the pickers list
    /// it; the person may choose another.
    suggested: Option<String>,
    _subscriptions: Vec<Subscription>,
}

/// Where the count stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Preview {
    /// A row has no target yet.
    Waiting,
    Counting,
    /// The total and each row's share.
    Count(u64, Vec<u64>),
}

/// One scope the dialog offers.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Within {
    All,
    Region(String),
    Slice(Cells),
    Selection(Cells),
}

pub(crate) struct ReplaceDialog {
    studio: WeakEntity<StudioView>,
    status: Entity<Submission>,
    file: String,
    blocks: Vec<BlockCount>,
    names: Rc<BlockNames>,
    schematic: Loaded<Arc<UniversalSchematic>>,
    /// The file's game version's blocks: the targets, and what a target
    /// keeps from the block it replaces.
    catalog: Loaded<Arc<BlockCatalog>>,
    schemes: ReplaceSchemes,
    rows: Vec<Row>,
    scopes: Vec<(Within, String)>,
    scope: usize,
    /// The scope's dropdown, when there is more than the whole file.
    scope_pick: Option<ScopeDropdown>,
    scheme_name: Entity<InputState>,
    /// 存为方案…'s popover, asking for the name, is open.
    naming: bool,
    /// The 方案 the table came from, while it is unchanged.
    from_scheme: Option<String>,
    /// New recent targets or a block table came: the target pickers are
    /// listed again on the next draw, which has the window.
    refill: bool,
    pub(crate) preview: Preview,
    generation: u64,
    counting: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

/// What opens the dialog, and what its first row starts from.
pub(crate) struct Opening {
    pub(crate) file: String,
    pub(crate) bytes: Arc<Vec<u8>>,
    pub(crate) blocks: Vec<BlockCount>,
    /// What the pickers call the blocks.
    pub(crate) names: Rc<BlockNames>,
    pub(crate) regions: Vec<String>,
    pub(crate) selection: Option<Cells>,
    pub(crate) slice: Option<Cells>,
    pub(crate) schemes: ReplaceSchemes,
}

impl ReplaceDialog {
    pub(crate) fn new(
        studio: WeakEntity<StudioView>,
        opening: Opening,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let Opening {
            file,
            bytes,
            blocks,
            names,
            regions,
            selection,
            slice,
            schemes,
        } = opening;
        let mut scopes = vec![(Within::All, tr!("studio-replace-all-regions").to_owned())];
        if regions.len() > 1 {
            scopes.extend(
                regions
                    .into_iter()
                    .map(|name| (Within::Region(name.clone()), name)),
            );
        }
        if let Some(cells) = slice {
            scopes.push((
                Within::Slice(cells),
                tr!("studio-replace-scope-slice").to_owned(),
            ));
        }
        if let Some((min, max)) = selection {
            let side = |axis: usize| max[axis] - min[axis] + 1;
            scopes.push((
                Within::Selection((min, max)),
                tr!(
                    "studio-replace-selection",
                    x = side(0),
                    y = side(1),
                    z = side(2)
                )
                .to_owned(),
            ));
        }
        // A box selected in the picture is what a replacement opened with
        // it is about (T25).
        let scope = scopes
            .iter()
            .position(|(within, _)| matches!(within, Within::Selection(_)))
            .unwrap_or(0);
        let scheme_name = cx.new(|cx| InputState::new(window, cx));
        let mut subscriptions = vec![cx.subscribe(
            &scheme_name,
            |this, _, event: &InputEvent, cx| match event {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } => this.save_scheme(cx),
                _ => {}
            },
        )];
        let scope_pick = (scopes.len() > 1).then(|| {
            let choices: Vec<ScopeChoice> = scopes
                .iter()
                .enumerate()
                .map(|(index, (_, label))| ScopeChoice {
                    index,
                    label: label.clone().into(),
                })
                .collect();
            let dropdown = cx.new(|cx| {
                SelectState::new(
                    SearchableVec::new(choices),
                    Some(IndexPath::new(scope)),
                    window,
                    cx,
                )
            });
            subscriptions.push(cx.subscribe_in(
                &dropdown,
                window,
                |this, _, event: &SelectEvent<SearchableVec<ScopeChoice>>, _, cx| {
                    if let SelectEvent::Confirm(Some(index)) = event {
                        this.scope = *index;
                        this.edited(cx);
                    }
                },
            ));
            dropdown
        });
        let status = cx.new(|_| Submission::default());
        let parse = cx
            .background_executor()
            .spawn(async move { lumilio_schematic::load_litematic(&bytes) });
        cx.spawn(async move |this, cx| {
            let parsed = parse.await;
            let _ = this.update(cx, |this, cx| {
                this.schematic = match parsed {
                    Ok(schematic) => Loaded::Ready(Arc::new(schematic)),
                    Err(error) => Loaded::Failed(error.to_string()),
                };
                this.refresh(cx);
            });
        })
        .detach();
        let mut dialog = Self {
            studio,
            status,
            file,
            blocks,
            names,
            schematic: Loaded::Loading,
            catalog: Loaded::Loading,
            schemes: ReplaceSchemes::default(),
            rows: Vec::new(),
            scopes,
            scope,
            scope_pick,
            scheme_name,
            naming: false,
            from_scheme: None,
            refill: false,
            preview: Preview::Waiting,
            generation: 0,
            counting: None,
            _subscriptions: Vec::new(),
        };
        dialog.schemes = schemes;
        if let Some(first) = dialog.blocks.first().map(|block| block.id.clone()) {
            dialog.add_row(&first, None, window, cx);
        }
        subscriptions.extend(std::mem::take(&mut dialog._subscriptions));
        dialog._subscriptions = subscriptions;
        dialog
    }

    /// The saved tables and recent targets, as the studio has them now.
    pub(crate) fn set_schemes(&mut self, schemes: ReplaceSchemes, cx: &mut Context<Self>) {
        self.schemes = schemes;
        self.refill = true;
        cx.notify();
    }

    /// The file's game version's blocks arrived, or could not be had.
    pub(crate) fn set_catalog(
        &mut self,
        catalog: Result<BlockCatalog, String>,
        cx: &mut Context<Self>,
    ) {
        self.catalog = match catalog {
            Ok(catalog) => Loaded::Ready(Arc::new(catalog)),
            Err(detail) => Loaded::Failed(detail),
        };
        self.refill = true;
        self.refresh(cx);
    }

    /// Whether the dialog still waits for its block table.
    pub(crate) fn wants_catalog(&self) -> bool {
        matches!(self.catalog, Loaded::Loading)
    }

    /// The targets on offer: the recent ones first, then every block the
    /// game version has (the file's own while that is not known).
    fn targets(&self) -> Vec<String> {
        let mut all: Vec<String> = match &self.catalog {
            Loaded::Ready(catalog) => catalog.ids().map(str::to_owned).collect(),
            _ => self.blocks.iter().map(|block| block.id.clone()).collect(),
        };
        all.sort();
        let recent: Vec<String> = self
            .schemes
            .recent
            .iter()
            .filter(|id| all.contains(id))
            .cloned()
            .collect();
        all.retain(|id| !recent.contains(id));
        recent.into_iter().chain(all).collect()
    }

    /// Lists the targets again; a row with nothing chosen takes its
    /// proposal once the list has it. Returns whether one was taken.
    fn refill_targets(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let ids = self.targets();
        let targets = BlockChoice::all(&self.names, &ids);
        let mut proposed = false;
        for row in &self.rows {
            let chosen = row.target.read(cx).selected_value().cloned();
            let wanted = chosen.or_else(|| {
                let take = row.suggested.clone().filter(|id| ids.contains(id));
                proposed |= take.is_some();
                take
            });
            row.target.update(cx, |target, cx| {
                target.set_items(SearchableVec::new(targets.clone()), window, cx);
                if let Some(wanted) = &wanted {
                    target.set_selected_value(wanted, window, cx);
                }
            });
        }
        proposed
    }

    /// A row starting from block `id`: with `exact` its properties match
    /// those values (from the picture), else any state of it.
    fn add_row(
        &mut self,
        id: &str,
        exact: Option<&[(String, String)]>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ids: Vec<String> = self.blocks.iter().map(|block| block.id.clone()).collect();
        let at = ids.iter().position(|known| known == id).map(IndexPath::new);
        let sources = BlockChoice::all(&self.names, &ids);
        let targets = BlockChoice::all(&self.names, &self.targets());
        let source = cx.new(|cx| {
            SelectState::new(SearchableVec::new(sources), at, window, cx).searchable(true)
        });
        let target = cx.new(|cx| {
            SelectState::new(SearchableVec::new(targets), None, window, cx).searchable(true)
        });
        let index = self.rows.len();
        let subscriptions = vec![
            cx.subscribe_in(
                &source,
                window,
                move |this, _, event: &SelectEvent<SearchableVec<BlockChoice>>, window, cx| {
                    if let SelectEvent::Confirm(Some(id)) = event {
                        this.choose_source(index, &id.clone(), window, cx);
                    }
                },
            ),
            cx.subscribe_in(
                &target,
                window,
                |this, _, event: &SelectEvent<SearchableVec<BlockChoice>>, _, cx| {
                    if matches!(event, SelectEvent::Confirm(_)) {
                        this.edited(cx);
                    }
                },
            ),
        ];
        self.rows.push(Row {
            source,
            any: exact.is_none(),
            chips: Vec::new(),
            target,
            suggested: None,
            _subscriptions: subscriptions,
        });
        self.set_chips(index, id, exact.unwrap_or(&[]), window, cx);
    }

    /// The chips of row `index` for block `id`: each property any state of
    /// it has in the file, at 任意 unless `exact` names its value.
    fn set_chips(
        &mut self,
        index: usize,
        id: &str,
        exact: &[(String, String)],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut names: Vec<(String, Vec<String>)> = Vec::new();
        if let Some(block) = self.blocks.iter().find(|block| block.id == id) {
            for state in &block.states {
                for (key, value) in &state.properties {
                    let at = match names.iter().position(|(name, _)| name == key) {
                        Some(at) => at,
                        None => {
                            names.push((key.clone(), Vec::new()));
                            names.len() - 1
                        }
                    };
                    if !names[at].1.contains(value) {
                        names[at].1.push(value.clone());
                    }
                }
            }
        }
        names.sort_by(|a, b| a.0.cmp(&b.0));
        let chips = names
            .into_iter()
            .enumerate()
            .map(|(slot, (name, mut values))| {
                values.sort();
                let labels: Vec<String> = std::iter::once(tr!("studio-replace-any").to_owned())
                    .chain(values)
                    .collect();
                let chosen = exact
                    .iter()
                    .find(|(key, _)| *key == name)
                    .and_then(|(_, value)| labels.iter().position(|label| label == value))
                    .unwrap_or(0);
                let pick = self.pick(labels, chosen, index, slot, window, cx);
                Chip { name, pick }
            })
            .collect();
        if let Some(row) = self.rows.get_mut(index) {
            row.chips = chips;
        }
    }

    fn pick(
        &mut self,
        labels: Vec<String>,
        chosen: usize,
        row: usize,
        slot: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Pick {
        let searchable = labels.len() > SEARCH_PAST;
        let dropdown = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(labels.clone()),
                Some(IndexPath::new(chosen)),
                window,
                cx,
            )
            .searchable(searchable)
        });
        self._subscriptions.push(cx.subscribe_in(
            &dropdown,
            window,
            move |this, _, event: &SelectEvent<SearchableVec<String>>, _, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    this.set_chip_value(row, slot, value, cx);
                }
            },
        ));
        Pick {
            labels,
            chosen,
            dropdown,
        }
    }

    fn set_chip_value(&mut self, row: usize, slot: usize, value: &str, cx: &mut Context<Self>) {
        if let Some(pick) = self
            .rows
            .get_mut(row)
            .and_then(|row| row.chips.get_mut(slot))
            .map(|chip| &mut chip.pick)
            && let Some(index) = pick.labels.iter().position(|label| label == value)
        {
            pick.chosen = index;
        }
        self.edited(cx);
    }

    /// Row `index`'s source changed: its chips start at 任意.
    fn choose_source(
        &mut self,
        index: usize,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_chips(index, id, &[], window, cx);
        self.edited(cx);
    }

    /// Something the person changed: the table is no longer the 方案 it
    /// came from, and the count is redone.
    fn edited(&mut self, cx: &mut Context<Self>) {
        self.from_scheme = None;
        self.refresh(cx);
    }

    /// Starts from one block state (a material's detail, a block in the
    /// picture): `id`, and with `properties` each matched exactly; a bare
    /// id matches any of its states.
    pub(crate) fn preset(
        &mut self,
        id: &str,
        properties: &[(String, String)],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.blocks.iter().any(|block| block.id == id) {
            return;
        }
        self.rows.clear();
        let exact = (!properties.is_empty()).then_some(properties);
        self.add_row(id, exact, window, cx);
        self.refresh(cx);
    }

    /// Starts from several blocks, one row each at any state, each waiting
    /// for its target (the convert dialog's kept blocks, T23) or starting
    /// from the one `suggested` proposes for it (T37).
    pub(crate) fn preset_many(
        &mut self,
        ids: &[String],
        suggested: &super::Suggested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let known: Vec<&String> = ids
            .iter()
            .filter(|id| self.blocks.iter().any(|block| block.id == **id))
            .collect();
        if known.is_empty() {
            return;
        }
        self.rows.clear();
        for id in known {
            self.add_row(id, None, window, cx);
            if let Some(row) = self.rows.last_mut() {
                row.suggested = suggested.get(id).cloned();
            }
        }
        self.refill_targets(window, cx);
        self.refresh(cx);
    }

    /// Replaces the table with the saved 方案 `name`; a row whose block the
    /// file does not have stays, and changes nothing here.
    pub(crate) fn apply_scheme(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rows) = self.schemes.schemes.get(name).cloned() else {
            return;
        };
        self.rows.clear();
        for saved in &rows {
            let exact = (!saved.properties.is_empty()).then_some(saved.properties.as_slice());
            self.add_row(&saved.source, exact, window, cx);
            if let Some(row) = self.rows.last() {
                let target = saved.target.clone();
                row.target.update(cx, |picker, cx| {
                    picker.set_selected_value(&target, window, cx);
                });
                if !self.blocks.iter().any(|block| block.id == saved.source) {
                    let only = BlockChoice::new(&self.names, &saved.source);
                    row.source.update(cx, |picker, cx| {
                        picker.set_items(SearchableVec::new(vec![only]), window, cx);
                        picker.set_selected_value(&saved.source, window, cx);
                    });
                }
            }
        }
        self.from_scheme = Some(name.to_owned());
        self.refresh(cx);
    }

    /// Row `index` as the core reads it, when it has a source and target.
    fn row_plan(&self, row: &Row, cx: &App) -> Option<(MappingRow, SchemeRow)> {
        let id = row.source.read(cx).selected_value()?.clone();
        let target = row.target.read(cx).selected_value()?.clone();
        let exact: Vec<(String, String)> = if row.any {
            Vec::new()
        } else {
            row.chips
                .iter()
                .filter(|chip| chip.pick.chosen > 0)
                .map(|chip| {
                    (
                        chip.name.clone(),
                        chip.pick.labels[chip.pick.chosen].clone(),
                    )
                })
                .collect()
        };
        let source = StateMatcher {
            id: id.clone(),
            properties: exact
                .iter()
                .map(|(name, value)| (name.clone(), PropertyMatch::Exact(value.clone())))
                .collect(),
        };
        Some((
            MappingRow {
                source,
                target: target.clone(),
                fixed: Vec::new(),
            },
            SchemeRow {
                source: id,
                properties: exact,
                target,
            },
        ))
    }

    fn scope(&self) -> Scope {
        match self.scopes.get(self.scope).map(|(within, _)| within) {
            Some(Within::Region(name)) => Scope::Region(name.clone()),
            Some(Within::Slice((min, max)) | Within::Selection((min, max))) => Scope::Cells {
                min: *min,
                max: *max,
            },
            Some(Within::All) | None => Scope::All,
        }
    }

    /// The table as the core reads it, when every row is complete.
    pub(crate) fn plan(&self, cx: &App) -> Option<Mapping> {
        let rows = self
            .rows
            .iter()
            .map(|row| self.row_plan(row, cx).map(|(row, _)| row))
            .collect::<Option<Vec<_>>>()?;
        (!rows.is_empty()).then(|| Mapping {
            rows,
            scope: self.scope(),
        })
    }

    fn scheme_rows(&self, cx: &App) -> Option<Vec<SchemeRow>> {
        self.rows
            .iter()
            .map(|row| self.row_plan(row, cx).map(|(_, saved)| saved))
            .collect()
    }

    fn catalog(&self) -> Option<Arc<BlockCatalog>> {
        match &self.catalog {
            Loaded::Ready(catalog) => Some(catalog.clone()),
            _ => None,
        }
    }

    /// Counts again on the background executor; an older count that lands
    /// later is dropped.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        self.counting = None;
        self.status.update(cx, |status, cx| status.clear(cx));
        let plan = self.plan(cx);
        self.preview = match (&self.schematic, plan) {
            (Loaded::Ready(schematic), Some(plan)) => {
                let (schematic, generation, catalog) =
                    (schematic.clone(), self.generation, self.catalog());
                let count = cx.background_executor().spawn(async move {
                    lumilio_schematic::replace::mapping_count(&schematic, &plan, catalog.as_deref())
                });
                self.counting = Some(cx.spawn(async move |this, cx| {
                    let (total, rows) = count.await;
                    let _ = this.update(cx, |this, cx| {
                        if this.generation == generation {
                            this.preview = Preview::Count(total, rows);
                            cx.notify();
                        }
                    });
                }));
                Preview::Counting
            }
            _ => Preview::Waiting,
        };
        cx.notify();
    }

    /// Chooses `id` as row `row`'s target, as a person would.
    #[cfg(test)]
    pub(crate) fn set_target_for_test(
        &mut self,
        row: usize,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(target) = self.rows.get(row).map(|row| row.target.clone()) {
            target.update(cx, |target, cx| {
                target.set_selected_value(&id.to_owned(), window, cx)
            });
        }
        self.edited(cx);
    }

    /// Sets 不限属性 on row `row`.
    pub(crate) fn set_any(&mut self, row: usize, any: bool, cx: &mut Context<Self>) {
        if let Some(row) = self.rows.get_mut(row) {
            row.any = any;
        }
        self.edited(cx);
    }

    /// The source block of the first row, if any.
    #[cfg(test)]
    pub(crate) fn source_id(&self, cx: &App) -> Option<String> {
        self.rows
            .first()
            .and_then(|row| row.source.read(cx).selected_value().cloned())
    }

    /// Each row's source id, in order.
    #[cfg(test)]
    pub(crate) fn sources(&self, cx: &App) -> Vec<Option<String>> {
        self.rows
            .iter()
            .map(|row| row.source.read(cx).selected_value().cloned())
            .collect()
    }

    /// Each row's target id, in order.
    #[cfg(test)]
    pub(crate) fn chosen_targets(&self, cx: &App) -> Vec<Option<String>> {
        self.rows
            .iter()
            .map(|row| row.target.read(cx).selected_value().cloned())
            .collect()
    }

    /// How each row's chips stand: its 不限属性 and `name=value` (or
    /// `name=*` for 任意).
    #[cfg(test)]
    pub(crate) fn chips(&self) -> Vec<(bool, Vec<String>)> {
        self.rows
            .iter()
            .map(|row| {
                (
                    row.any,
                    row.chips
                        .iter()
                        .map(|chip| match chip.pick.chosen {
                            0 => format!("{}=*", chip.name),
                            at => format!("{}={}", chip.name, chip.pick.labels[at]),
                        })
                        .collect(),
                )
            })
            .collect()
    }

    /// The 方案 name field.
    #[cfg(test)]
    pub(crate) fn scheme_name_for_test(&self) -> Entity<InputState> {
        self.scheme_name.clone()
    }

    /// Keeps the table as 方案 `name` in launcher data.
    fn save_scheme(&mut self, cx: &mut Context<Self>) {
        let name = self.scheme_name.read(cx).value().trim().to_owned();
        let Some(rows) = self.scheme_rows(cx).filter(|_| !name.is_empty()) else {
            return;
        };
        let mut schemes = self.schemes.clone();
        schemes.schemes.insert(name.clone(), rows);
        self.schemes = schemes.clone();
        self.from_scheme = Some(name);
        self.naming = false;
        let _ = self
            .studio
            .update(cx, |studio, _| studio.save_schemes(schemes));
        cx.notify();
    }

    /// Whether the table can be kept as a 方案: every row is complete.
    fn can_save(&self, cx: &App) -> bool {
        self.scheme_rows(cx).is_some()
    }

    /// 套用方案…'s entries: one per kept 方案, by name.
    pub(crate) fn scheme_entries(&self, cx: &mut Context<Self>) -> Vec<MenuEntry> {
        self.schemes
            .schemes
            .keys()
            .map(|name| {
                let (this, name) = (cx.weak_entity(), name.clone());
                MenuEntry::new(name.clone(), move |window, cx| {
                    let _ = this.update(cx, |this, cx| this.apply_scheme(&name, window, cx));
                })
            })
            .collect()
    }

    /// 存为方案…'s popover: the name and the key that keeps it.
    fn naming_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = ShellColors::current(cx);
        let can_save = !self.scheme_name.read(cx).value().trim().is_empty() && self.can_save(cx);
        v_flex()
            .debug_selector(|| "studio-replace-scheme-naming".into())
            .w(px(260.))
            .gap_2()
            .child(field(
                tr!("studio-replace-scheme-name"),
                div()
                    .w_full()
                    .debug_selector(|| "studio-replace-scheme-name".into())
                    .child(Input::new(&self.scheme_name).small()),
                colors,
            ))
            .child(
                h_flex().w_full().justify_end().child(
                    Key::new("studio-replace-scheme-keep")
                        .label(tr!("studio-replace-scheme-keep"))
                        .white()
                        .small()
                        .disabled(!can_save)
                        .debug_selector(|| "studio-replace-scheme-keep".into())
                        .on_click(cx.listener(|this, _, _, cx| this.save_scheme(cx))),
                ),
            )
            .into_any_element()
    }

    fn row_view(
        &self,
        index: usize,
        row: &Row,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let busy = self.status.read(cx).busy;
        let count = match &self.preview {
            Preview::Count(_, rows) => rows
                .get(index)
                .map(|count| tr!("studio-replace-row-count", count = *count)),
            _ => None,
        };
        let this = cx.weak_entity();
        let remove = (self.rows.len() > 1).then(|| {
            Key::new(("studio-replace-remove", index))
                .icon(gpui_component::Icon::new(crate::assets::UiIcon::Close))
                .ghost()
                .small()
                .tooltip(tr!("studio-replace-remove-row"))
                .debug_selector(move || format!("studio-replace-remove-{index}"))
                .on_click(move |_, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        if index < this.rows.len() && this.rows.len() > 1 {
                            this.rows.remove(index);
                            this.edited(cx);
                        }
                    });
                })
        });
        let this = cx.weak_entity();
        let any = Fader::new(
            ("studio-replace-any", index),
            row.any,
            tr!("studio-replace-any-props"),
            move |_, cx| {
                let _ = this.update(cx, |this, cx| {
                    let any = this.rows.get(index).is_some_and(|row| !row.any);
                    this.set_any(index, any, cx);
                });
            },
        )
        .shown();
        let chips: Vec<AnyElement> = if row.any {
            Vec::new()
        } else {
            row.chips
                .iter()
                .enumerate()
                .map(|(slot, chip)| {
                    // One line each, the name leading and the dropdown on
                    // the column's trailing edge, so the dropdowns stand in
                    // one column whatever the names' lengths.
                    h_flex()
                        .debug_selector(move || format!("studio-replace-chip-{index}-{slot}"))
                        .w_full()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .font_family(theme::mono_font())
                                .text_xs()
                                .text_color(colors.muted)
                                .child(chip.name.clone()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .w(px(CHIP_WIDTH))
                                .child(Select::new(&chip.pick.dropdown).small().disabled(busy)),
                        )
                        .into_any_element()
                })
                .collect()
        };
        // The arrow sits in a lane a small control tall, on the pickers'
        // line. The target picker runs to the table's trailing edge, or to
        // ✕, which every row has once there are two; the row's count is
        // its caption, as the chips are the source's.
        let lane = || {
            h_flex()
                .flex_none()
                .h(px(KeySize::Small.height()))
                .items_center()
        };
        h_flex()
            .debug_selector(move || format!("studio-replace-row-{index}"))
            .w_full()
            .gap_2()
            .items_start()
            .py(px(8.))
            .when(index > 0, |row| {
                row.border_t_1().border_color(colors.border)
            })
            .child(
                // The source, and under it what it matches: its properties
                // stay in the source's column.
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_2()
                    .child(
                        div()
                            .w_full()
                            .debug_selector(move || format!("studio-replace-source-{index}"))
                            .child(
                                Select::new(&row.source)
                                    .small()
                                    .disabled(busy)
                                    .menu_width(px(BLOCK_MENU))
                                    .search_placeholder(tr!("studio-blocks-placeholder")),
                            ),
                    )
                    .child(
                        div()
                            .debug_selector(move || format!("studio-replace-any-{index}"))
                            .child(any),
                    )
                    .children(chips),
            )
            .child(lane().text_sm().text_color(colors.muted).child("→"))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_2()
                    .child(
                        div()
                            .w_full()
                            .debug_selector(move || format!("studio-replace-target-{index}"))
                            .child(
                                Select::new(&row.target)
                                    .small()
                                    .disabled(busy)
                                    .menu_width(px(BLOCK_MENU))
                                    .placeholder(tr!("studio-replace-target-pick"))
                                    .search_placeholder(tr!("studio-blocks-placeholder")),
                            ),
                    )
                    .children(count.map(|count| {
                        div()
                            .debug_selector(move || format!("studio-replace-count-{index}"))
                            .font_family(theme::mono_font())
                            .text_xs()
                            .text_color(colors.foreground)
                            .child(count)
                    })),
            )
            .children(remove)
            .into_any_element()
    }
}

impl DialogForm for ReplaceDialog {
    fn status(&self) -> &Entity<Submission> {
        &self.status
    }

    fn ready(&self, _: &App) -> bool {
        matches!(self.preview, Preview::Count(count, _) if count > 0)
    }

    fn commit_label(&self) -> SharedString {
        match self.preview {
            Preview::Count(count, _) if count > 0 => {
                tr!("studio-replace-commit-count", count = count).into()
            }
            _ => tr!("studio-replace-commit").into(),
        }
    }

    fn width(&self) -> f32 {
        640.
    }

    fn footer_start(&mut self, _: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let colors = ShellColors::current(cx);
        let busy = self.status.read(cx).busy;
        // ia[plugin.litematica]: 套用方案 | 替换对话框底栏左侧「套用方案…」（存过方案时才有）→ 菜单列出方案名 | 把表格换成方案的行，这份投影没有的方块那一行不替换任何东西；底栏写「方案「名」」直到表格被改动；用方案替换时历史行写方案名 | 方案存在启动器数据里，所有实例共用（T24）
        let apply = (!self.schemes.schemes.is_empty()).then(|| {
            kit::with_menu(
                Key::new("studio-replace-scheme-pick")
                    .label(tr!("studio-replace-scheme-pick"))
                    .white()
                    .disabled(busy)
                    .debug_selector(|| "studio-replace-scheme-pick".into()),
                self.scheme_entries(cx),
                Anchor::BottomLeft,
                colors,
            )
            .into_any_element()
        });
        let save_key = Key::new("studio-replace-scheme-save")
            .label(tr!("studio-replace-scheme-save"))
            .white()
            .debug_selector(|| "studio-replace-scheme-save".into());
        // ia[plugin.litematica]: 存为方案 | 替换对话框底栏左侧「存为方案…」→ 弹出框里的方案名与「存」（回车也行） | 把表格存成这个名字的方案（同名覆盖），弹出框关上，底栏写「方案「名」」；之后在任何投影的替换对话框里都能套用 | 每一行都选好目标方块之前置灰并说明；每次替换也记下用过的目标方块，下次排在选择器最前（T24）
        let save = if self.can_save(cx) && !busy {
            let (this, panel) = (cx.weak_entity(), cx.weak_entity());
            Popover::new("studio-replace-scheme-popover")
                .anchor(Anchor::BottomLeft)
                .open(self.naming)
                .on_open_change(move |open, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.naming = *open;
                        cx.notify();
                    });
                })
                .track_focus(&self.scheme_name.read(cx).focus_handle(cx))
                .trigger(save_key)
                .content(move |_, _, cx| {
                    panel
                        .update(cx, |this, cx| this.naming_panel(cx))
                        .unwrap_or_else(|_| div().into_any_element())
                })
                .into_any_element()
        } else {
            save_key
                .disabled(true)
                .tooltip(tr!("studio-replace-scheme-unready"))
                .into_any_element()
        };
        let from = self.from_scheme.as_deref().map(|name| {
            div()
                .debug_selector(|| "studio-replace-scheme-current".into())
                .min_w_0()
                .truncate()
                .text_sm()
                .text_color(colors.muted)
                .child(tr!("studio-replace-scheme-current", name = name))
        });
        Some(
            h_flex()
                .min_w_0()
                .gap_2()
                .items_center()
                .children(apply)
                .child(save)
                .children(from)
                .into_any_element(),
        )
    }

    fn commit(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let (Some(plan), Some(saved)) = (self.plan(cx), self.scheme_rows(cx)) else {
            return;
        };
        let catalog = self.catalog();
        let summary_rows: Vec<(String, String)> = saved
            .iter()
            .map(|row| {
                let source = if row.properties.is_empty() {
                    row.source.clone()
                } else {
                    let inner: Vec<String> = row
                        .properties
                        .iter()
                        .map(|(key, value)| format!("{key}={value}"))
                        .collect();
                    format!("{}[{}]", row.source, inner.join(","))
                };
                (source, row.target.clone())
            })
            .collect();
        let scheme = self.from_scheme.clone();
        let name = tree::stem(&self.file).to_owned();
        let status = self.status.clone();
        let mut schemes = self.schemes.clone();
        schemes.used(saved.iter().map(|row| row.target.clone()));
        let _ = self
            .studio
            .update(cx, |studio, _| studio.save_schemes(schemes));
        let edit = move |bytes: &[u8]| {
            let mut schematic =
                lumilio_schematic::load_litematic(bytes).map_err(|error| error.to_string())?;
            let count = lumilio_schematic::replace::apply_mapping(
                &mut schematic,
                &plan,
                catalog.as_deref(),
            );
            let bytes = lumilio_schematic::to_litematic_bytes(&schematic)
                .map_err(|error| error.to_string())?;
            Ok(Edited { bytes, count })
        };
        let describe = Box::new(move |count: u64| {
            let summary = match (&scheme, summary_rows.as_slice()) {
                (Some(scheme), _) => tr!(
                    "studio-replace-summary-scheme",
                    name = scheme.as_str(),
                    count = count
                ),
                (None, [(source, target)]) => tr!(
                    "studio-replace-summary",
                    source = source.as_str(),
                    target = target.as_str(),
                    count = count
                ),
                (None, rows) => tr!(
                    "studio-replace-summary-rows",
                    rows = rows.len(),
                    count = count
                ),
            };
            (
                SchematicOperation {
                    kind: "replace".into(),
                    summary,
                },
                tr!("studio-replaced", count = count, file = name.as_str()),
            )
        });
        let _ = self.studio.update(cx, |studio, cx| {
            studio.submit(status, edit, describe, cx);
        });
    }
}

impl Render for ReplaceDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.refill) && self.refill_targets(window, cx) {
            self.refresh(cx);
        }
        let colors = ShellColors::current(cx);
        let busy = self.status.read(cx).busy;
        let rows: Vec<AnyElement> = (0..self.rows.len())
            .map(|index| {
                let row = &self.rows[index];
                self.row_view(index, row, colors, cx)
            })
            .collect();
        let this = cx.weak_entity();
        // ia[plugin.litematica]: 加一行映射 | 替换对话框表格下的「加一行」 | 多一行「从 → 到」，从文件里下一种还没有行的方块开始；每行旁边写它会替换多少个，确认键写总数 | 各行一起按替换前的样子匹配，不会一行接着一行替换（T24）
        let add = Key::new("studio-replace-add-row")
            .label(tr!("studio-replace-add-row"))
            .ghost()
            .small()
            .flush_start()
            .disabled(busy)
            .debug_selector(|| "studio-replace-add-row".into())
            .on_click(move |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    let used: Vec<String> = this
                        .rows
                        .iter()
                        .filter_map(|row| row.source.read(cx).selected_value().cloned())
                        .collect();
                    let next = this
                        .blocks
                        .iter()
                        .map(|block| block.id.clone())
                        .find(|id| !used.contains(id))
                        .or_else(|| this.blocks.first().map(|block| block.id.clone()));
                    if let Some(next) = next {
                        this.add_row(&next, None, window, cx);
                        this.edited(cx);
                    }
                });
            });
        // The table is the dialog's subject, named by its title: each row's
        // arrow and the target's placeholder say which side is which.
        let mut column = v_flex().w_full().gap_3().child(
            v_flex()
                .w_full()
                .gap_1()
                .child(
                    v_flex()
                        .debug_selector(|| "studio-replace-rows".into())
                        .w_full()
                        .children(rows),
                )
                .child(h_flex().child(add)),
        );
        match &self.catalog {
            Loaded::Loading => {
                column = column.child(note(tr!("studio-replace-catalog-loading"), colors));
            }
            Loaded::Failed(_) => {
                column = column.child(
                    note(tr!("studio-replace-catalog-failed"), colors)
                        .debug_selector(|| "studio-replace-catalog-failed".into()),
                );
            }
            Loaded::Ready(_) => {}
        }
        if let Some(scope) = &self.scope_pick {
            // ia[plugin.litematica]: 选替换范围 | 替换对话框「范围」下拉（全部区域 / 各区域名 / 当前切片 / 选区） | 计数与替换只算范围里的方块 | 只有整份文件时不出现；有选区时默认选区；当前切片是视口正在看的那一层（T24、T25）
            column = column.child(field(
                tr!("studio-replace-scope"),
                div()
                    .w(px(280.))
                    .debug_selector(|| "studio-replace-scope".into())
                    .child(Select::new(scope).small().disabled(busy)),
                colors,
            ));
        }
        // What the count stands at, when the rows' counts and the commit
        // key's total do not already say it.
        let preview = match (&self.schematic, &self.preview) {
            (Loaded::Failed(_), _) => Some(note(tr!("studio-read-failed"), colors)),
            (Loaded::Loading, _) | (_, Preview::Counting) => {
                Some(note(tr!("studio-replace-counting"), colors))
            }
            (_, Preview::Waiting) => Some(note(tr!("studio-replace-waiting"), colors)),
            (_, Preview::Count(0, _)) => Some(note(tr!("studio-replace-none"), colors)),
            (_, Preview::Count(..)) => None,
        };
        column.children(
            preview.map(|preview| preview.debug_selector(|| "studio-replace-preview".into())),
        )
    }
}
