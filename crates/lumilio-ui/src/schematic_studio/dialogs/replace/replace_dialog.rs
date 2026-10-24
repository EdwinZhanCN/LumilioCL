use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{App, Context, Entity, SharedString, Subscription, Task, WeakEntity, Window, div};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::select::{SearchableVec, SelectEvent, SelectItem, SelectState};
use gpui_component::{IndexPath, h_flex};
use lumilio_core::{BlockCatalog, BlockNames, ReplaceSchemes};
use lumilio_schematic::UniversalSchematic;
use lumilio_schematic::stats::BlockCount;

use super::super::Submission;
use crate::model_view::Cells;
use crate::schematic_studio::words::block_name;
use crate::schematic_studio::{Loaded, StudioView};
use crate::theme::ShellColors;
use crate::{theme, tr};

/// A property chip's dropdown is searchable past this many values.
pub(super) const SEARCH_PAST: usize = 8;

/// A property chip's dropdown: room for values such as `ascending_east`.
pub(super) const CHIP_WIDTH: f32 = 140.;

/// A block picker's list: wider than half the dialog, so a name and its ID
/// both fit.
pub(super) const BLOCK_MENU: f32 = 360.;

pub(super) type Dropdown = Entity<SelectState<SearchableVec<String>>>;

/// The scope's dropdown.
pub(super) type ScopeDropdown = Entity<SelectState<SearchableVec<ScopeChoice>>>;

/// A row's source or target picker: blocks by name, chosen by ID.
pub(super) type BlockDropdown = Entity<SelectState<SearchableVec<BlockChoice>>>;

/// A block in a picker: its name in the launcher's language as the label,
/// its ID as the value and a mono caption (D28: an ID is the label only
/// when no name is known). The search finds either.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BlockChoice {
    pub(super) id: String,
    pub(super) name: SharedString,
}

impl BlockChoice {
    pub(crate) fn new(names: &BlockNames, id: &str) -> Self {
        Self {
            id: id.to_owned(),
            name: block_name(names, id).into(),
        }
    }

    pub(super) fn all(names: &BlockNames, ids: &[String]) -> Vec<Self> {
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
pub(super) struct ScopeChoice {
    pub(super) index: usize,
    pub(super) label: SharedString,
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
pub(super) struct Pick {
    pub(super) labels: Vec<String>,
    pub(super) chosen: usize,
    pub(super) dropdown: Dropdown,
}

/// One property chip of a row's source: index 0 is 任意.
pub(super) struct Chip {
    pub(super) name: String,
    pub(super) pick: Pick,
}

/// One row of the table.
pub(super) struct Row {
    pub(super) source: BlockDropdown,
    /// 不限属性: every state of the source block.
    pub(super) any: bool,
    pub(super) chips: Vec<Chip>,
    pub(super) target: BlockDropdown,
    /// A target proposed for this row (T37), chosen once the pickers list
    /// it; the person may choose another.
    pub(super) suggested: Option<String>,
    pub(super) _subscriptions: Vec<Subscription>,
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
pub(super) enum Within {
    All,
    Region(String),
    Slice(Cells),
    Selection(Cells),
}

pub(crate) struct ReplaceDialog {
    pub(super) studio: WeakEntity<StudioView>,
    pub(super) status: Entity<Submission>,
    pub(super) file: String,
    pub(super) blocks: Vec<BlockCount>,
    pub(super) names: Rc<BlockNames>,
    pub(super) schematic: Loaded<Arc<UniversalSchematic>>,
    /// The file's game version's blocks: the targets, and what a target
    /// keeps from the block it replaces.
    pub(super) catalog: Loaded<Arc<BlockCatalog>>,
    pub(super) schemes: ReplaceSchemes,
    pub(super) rows: Vec<Row>,
    pub(super) scopes: Vec<(Within, String)>,
    pub(super) scope: usize,
    /// The scope's dropdown, when there is more than the whole file.
    pub(super) scope_pick: Option<ScopeDropdown>,
    pub(super) scheme_name: Entity<InputState>,
    /// 存为方案…'s popover, asking for the name, is open.
    pub(super) naming: bool,
    /// The 方案 the table came from, while it is unchanged.
    pub(super) from_scheme: Option<String>,
    /// New recent targets or a block table came: the target pickers are
    /// listed again on the next draw, which has the window.
    pub(super) refill: bool,
    pub(crate) preview: Preview,
    pub(super) generation: u64,
    pub(super) counting: Option<Task<()>>,
    pub(super) _subscriptions: Vec<Subscription>,
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
    pub(super) fn targets(&self) -> Vec<String> {
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
    pub(super) fn refill_targets(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
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
    pub(super) fn add_row(
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
    pub(super) fn set_chips(
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

    pub(super) fn pick(
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

    pub(super) fn set_chip_value(
        &mut self,
        row: usize,
        slot: usize,
        value: &str,
        cx: &mut Context<Self>,
    ) {
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
    pub(super) fn choose_source(
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
    pub(super) fn edited(&mut self, cx: &mut Context<Self>) {
        self.from_scheme = None;
        self.refresh(cx);
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
}
