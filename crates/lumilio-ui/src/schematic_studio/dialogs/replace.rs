//! 替换方块… (plan schematic-studio, T13, T24): a table of rows, 从 → 到.
//! Each row's source is one of the file's blocks with a chip per property
//! (a value, or 任意; 不限属性 drops them all); its target is chosen from
//! the file's game version's blocks, recent ones first, and keeps every
//! property it shares with the block it replaces. The scope is all,
//! one region, the current slice or the current selection. What would
//! change is counted on the background executor whenever anything changes,
//! row by row, and the commit key says the total. A table can be kept as a
//! named 方案 in launcher data and applied to another schematic.

use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, SharedString, Subscription, Task, WeakEntity, Window, div, px,
};
use gpui_component::IndexPath;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::select::{SearchableVec, Select, SelectEvent, SelectItem, SelectState};
use gpui_component::{Sizable as _, h_flex, v_flex};
use lumilio_core::{BlockCatalog, BlockNames, ReplaceSchemes, SchematicOperation, SchemeRow};
use lumilio_schematic::UniversalSchematic;
use lumilio_schematic::replace::{Mapping, MappingRow, PropertyMatch, Scope, StateMatcher};
use lumilio_schematic::stats::BlockCount;

use super::{DialogForm, Submission, field, note};
use crate::controls::{Fader, Segments};
use crate::key::Key;
use crate::model_view::Cells;
use crate::schematic_studio::words::block_name;
use crate::schematic_studio::writes::Edited;
use crate::schematic_studio::{Loaded, StudioView, tree};
use crate::theme::{self, ShellColors};
use crate::tr;

/// A choice of up to this many options is a row of segment keys; a longer
/// one is a searchable dropdown (AGENTS.md control rules).
const SEGMENT_LIMIT: usize = 4;

type Dropdown = Entity<SelectState<SearchableVec<String>>>;

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

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = ShellColors::current(cx);
        h_flex()
            .w_full()
            .min_w_0()
            .gap_2()
            .justify_between()
            .child(div().min_w_0().truncate().child(self.name.clone()))
            .when(self.name.as_ref() != self.id, |row| {
                row.child(
                    div()
                        .flex_none()
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

/// The element id of row `row`'s chip `slot`, made once per pair:
/// [`Segments`] wants an id that lives as long as the program.
fn chip_id(row: usize, slot: usize) -> &'static str {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static IDS: OnceLock<Mutex<HashMap<(usize, usize), &'static str>>> = OnceLock::new();
    let mut ids = IDS
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    ids.entry((row, slot))
        .or_insert_with(|| Box::leak(format!("studio-replace-chip-{row}-{slot}").into_boxed_str()))
}

/// One choice among labels: segment keys for a few, a dropdown for more.
struct Pick {
    labels: Vec<String>,
    chosen: usize,
    dropdown: Option<Dropdown>,
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
    scheme_name: Entity<InputState>,
    scheme_pick: Option<Dropdown>,
    /// The 方案 the table came from, while it is unchanged.
    from_scheme: Option<String>,
    /// New schemes or a block table came: the pickers are rebuilt on the
    /// next draw, which has the window.
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
        let scheme_name =
            cx.new(|cx| InputState::new(window, cx).placeholder(tr!("studio-replace-scheme-name")));
        let mut subscriptions = vec![cx.subscribe(&scheme_name, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })];
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
            scheme_name,
            scheme_pick: None,
            from_scheme: None,
            refill: false,
            preview: Preview::Waiting,
            generation: 0,
            counting: None,
            _subscriptions: Vec::new(),
        };
        dialog.schemes = schemes;
        dialog.rebuild_scheme_pick(window, cx);
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

    fn rebuild_scheme_pick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let names: Vec<String> = self.schemes.schemes.keys().cloned().collect();
        self.scheme_pick = (!names.is_empty()).then(|| {
            let dropdown = cx.new(|cx| {
                SelectState::new(SearchableVec::new(names), None, window, cx).searchable(true)
            });
            self._subscriptions.push(cx.subscribe_in(
                &dropdown,
                window,
                |this, _, event: &SelectEvent<SearchableVec<String>>, window, cx| {
                    if let SelectEvent::Confirm(Some(name)) = event {
                        this.apply_scheme(&name.clone(), window, cx);
                    }
                },
            ));
            dropdown
        });
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
        if labels.len() <= SEGMENT_LIMIT {
            return Pick {
                labels,
                chosen,
                dropdown: None,
            };
        }
        let dropdown = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(labels.clone()),
                Some(IndexPath::new(chosen)),
                window,
                cx,
            )
            .searchable(true)
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
            dropdown: Some(dropdown),
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

    fn set_chip_index(&mut self, row: usize, slot: usize, index: usize, cx: &mut Context<Self>) {
        if let Some(pick) = self
            .rows
            .get_mut(row)
            .and_then(|row| row.chips.get_mut(slot))
            .map(|chip| &mut chip.pick)
        {
            pick.chosen = index.min(pick.labels.len().saturating_sub(1));
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
        self.from_scheme = Some(name);
        let _ = self
            .studio
            .update(cx, |studio, _| studio.save_schemes(schemes));
    }

    fn pick_control(
        &self,
        pick: &Pick,
        row: usize,
        slot: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(dropdown) = &pick.dropdown {
            return div()
                .min_w(px(120.))
                .child(Select::new(dropdown).small())
                .into_any_element();
        }
        let this = cx.weak_entity();
        Segments::new(
            chip_id(row, slot),
            &pick.labels,
            pick.chosen,
            move |index, _, cx| {
                let _ = this.update(cx, |this, cx| this.set_chip_index(row, slot, index, cx));
            },
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
                .label("✕")
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
                    h_flex()
                        .gap_1()
                        .items_center()
                        .child(
                            div()
                                .font_family(theme::mono_font())
                                .text_xs()
                                .text_color(colors.muted)
                                .child(format!("{} =", chip.name)),
                        )
                        .child(self.pick_control(&chip.pick, index, slot, cx))
                        .into_any_element()
                })
                .collect()
        };
        v_flex()
            .debug_selector(move || format!("studio-replace-row-{index}"))
            .w_full()
            .gap_2()
            .py(px(6.))
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .debug_selector(move || format!("studio-replace-source-{index}"))
                            .child(
                                Select::new(&row.source)
                                    .small()
                                    .disabled(busy)
                                    .search_placeholder(tr!("studio-blocks-placeholder")),
                            ),
                    )
                    .child(div().text_sm().text_color(colors.muted).child("→"))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .debug_selector(move || format!("studio-replace-target-{index}"))
                            .child(
                                Select::new(&row.target)
                                    .small()
                                    .disabled(busy)
                                    .placeholder(tr!("studio-replace-target-pick"))
                                    .search_placeholder(tr!("studio-blocks-placeholder")),
                            ),
                    )
                    .children(count.map(|count| {
                        div()
                            .flex_none()
                            .font_family(theme::mono_font())
                            .text_xs()
                            .text_color(colors.foreground)
                            .child(count)
                    }))
                    .children(remove),
            )
            .child(
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .debug_selector(move || format!("studio-replace-any-{index}"))
                            .child(any),
                    )
                    .children(chips),
            )
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
        if std::mem::take(&mut self.refill) {
            self.rebuild_scheme_pick(window, cx);
            if self.refill_targets(window, cx) {
                self.refresh(cx);
            }
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
        let mut column = v_flex()
            .w_full()
            .gap_3()
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(colors.muted)
                            .child(tr!("studio-replace-from")),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(colors.muted)
                            .child(tr!("studio-replace-to")),
                    ),
            )
            .child(
                v_flex()
                    .debug_selector(|| "studio-replace-rows".into())
                    .w_full()
                    .children(rows),
            )
            .child(div().child(add));
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
        if self.scopes.len() > 1 {
            let labels: Vec<String> = self.scopes.iter().map(|(_, label)| label.clone()).collect();
            let this = cx.weak_entity();
            // ia[plugin.litematica]: 选替换范围 | 替换对话框「范围」（全部 / 区域 / 当前切片 / 当前选区） | 计数与替换只算范围里的方块 | 有选区时默认选区；当前切片是视口正在看的那一层（T24、T25）
            let scope = Segments::new(
                "studio-replace-scope",
                &labels,
                self.scope,
                move |index, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.scope = index;
                        this.edited(cx);
                    });
                },
            );
            column = column.child(field(
                tr!("studio-replace-scope"),
                div()
                    .debug_selector(|| "studio-replace-scope".into())
                    .child(scope),
                colors,
            ));
        }
        let this = cx.weak_entity();
        let can_save =
            !self.scheme_name.read(cx).value().trim().is_empty() && self.scheme_rows(cx).is_some();
        // ia[plugin.litematica]: 存为方案 / 套用方案 | 替换对话框底部「方案」：名字输入框与「存为方案」，有存过的方案时还有「套用方案…」下拉 | 存进启动器数据（所有实例共用）；套用就把表格换成方案的行，这份投影没有的方块那一行不替换任何东西；用方案替换时历史行写方案名 | 每次替换也记下用过的目标方块，下次排在选择器最前（T24）
        let save = Key::new("studio-replace-scheme-save")
            .label(tr!("studio-replace-scheme-save"))
            .white()
            .small()
            .disabled(!can_save || busy)
            .debug_selector(|| "studio-replace-scheme-save".into())
            .on_click(move |_, _, cx| {
                let _ = this.update(cx, |this, cx| this.save_scheme(cx));
            });
        column = column.child(field(
            tr!("studio-replace-schemes"),
            h_flex()
                .w_full()
                .gap_2()
                .items_center()
                .children(self.scheme_pick.as_ref().map(|pick| {
                    div()
                        .w(px(180.))
                        .debug_selector(|| "studio-replace-scheme-pick".into())
                        .child(
                            Select::new(pick)
                                .small()
                                .placeholder(tr!("studio-replace-scheme-pick")),
                        )
                }))
                .child(
                    div()
                        .flex_1()
                        .debug_selector(|| "studio-replace-scheme-name".into())
                        .child(Input::new(&self.scheme_name).small().disabled(busy)),
                )
                .child(save),
            colors,
        ));
        let preview = match (&self.schematic, &self.preview) {
            (Loaded::Failed(_), _) => note(tr!("studio-read-failed"), colors),
            (Loaded::Loading, _) | (_, Preview::Counting) => {
                note(tr!("studio-replace-counting"), colors)
            }
            (_, Preview::Waiting) => note(tr!("studio-replace-waiting"), colors),
            (_, Preview::Count(0, _)) => note(tr!("studio-replace-none"), colors),
            (_, Preview::Count(count, _)) => div()
                .text_sm()
                .text_color(colors.foreground)
                .child(tr!("studio-replace-count", count = *count)),
        };
        column.child(
            div()
                .debug_selector(|| "studio-replace-preview".into())
                .child(preview),
        )
    }
}
