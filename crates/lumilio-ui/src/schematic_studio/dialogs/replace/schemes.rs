use std::sync::Arc;

use gpui::{App, Context, Window};
use gpui_component::select::SearchableVec;
use lumilio_core::{BlockCatalog, SchemeRow};
use lumilio_schematic::replace::{Mapping, MappingRow, PropertyMatch, Scope, StateMatcher};

use super::{BlockChoice, Preview, ReplaceDialog, Row, Within};
use crate::kit::MenuEntry;
use crate::schematic_studio::Loaded;

impl ReplaceDialog {
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
        suggested: &super::super::Suggested,
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
    pub(super) fn row_plan(&self, row: &Row, cx: &App) -> Option<(MappingRow, SchemeRow)> {
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

    pub(super) fn scope(&self) -> Scope {
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

    pub(super) fn scheme_rows(&self, cx: &App) -> Option<Vec<SchemeRow>> {
        self.rows
            .iter()
            .map(|row| self.row_plan(row, cx).map(|(_, saved)| saved))
            .collect()
    }

    pub(super) fn catalog(&self) -> Option<Arc<BlockCatalog>> {
        match &self.catalog {
            Loaded::Ready(catalog) => Some(catalog.clone()),
            _ => None,
        }
    }

    /// Counts again on the background executor; an older count that lands
    /// later is dropped.
    pub(super) fn refresh(&mut self, cx: &mut Context<Self>) {
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

    /// Keeps the table as 方案 `name` in launcher data.
    pub(super) fn save_scheme(&mut self, cx: &mut Context<Self>) {
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
    pub(super) fn can_save(&self, cx: &App) -> bool {
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
}
