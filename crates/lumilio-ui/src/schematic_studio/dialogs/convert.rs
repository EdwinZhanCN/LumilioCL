//! 转换版本… (plan schematic-studio, T14, T23): the target is chosen with
//! the P-VERSION picker from the block-asset bundle's versions, the game
//! version chosen first. The converted bytes are made on the background
//! executor as soon as a version is chosen, so a backward conversion's
//! losses are listed before anything is written: lost, approximated, and
//! kept (blocks the target does not know, written as they are), with a way
//! to choose replacements for those. A forward conversion is lossless. The
//! result is saved beside the original as `name (y).litematic` unless
//! 覆盖原文件 is switched on. Cancelling writes nothing.
//!
//! A backward conversion first asks for the block states the target version
//! knows (its bundle slice): Nucleation reports only what its converters
//! invert, so a block the older version never had is found by that check.
//! Such blocks are written as they are, as Nucleation does; the dialog says
//! the older game reads them as air. Without the slice the conversion is
//! still offered, with a note that the check could not run.

use std::collections::BTreeMap;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, ScrollHandle, SharedString, Subscription, WeakEntity, Window,
    div, px,
};
use gpui_component::{Sizable as _, v_flex};
use lumilio_core::BlockCatalog;
use lumilio_core::SchematicOperation;
use lumilio_schematic::convert::{
    Catalogs, Conversion, Direction, Loss, MISSING_BLOCK, Severity, UNKNOWN_PROPERTY, direction,
};

use super::{DialogForm, Submission, field, note};
use crate::controls::Fader;
use crate::key::Key;
use crate::kit;
use crate::schematic_studio::writes::Edited;
use crate::schematic_studio::{Loaded, StudioView, tree};
use crate::theme::{self, ShellColors};
use crate::tr;
use crate::version_picker::{Choice, Choices, PickerEvent, VersionPicker};

/// Entries listed per severity; the rest are counted.
const LISTED: usize = 100;

/// A proposed replacement for each block a target lacks, by ID (T37).
pub(crate) type Suggested = BTreeMap<String, String>;

/// A proposal from the target's blocks for each block it lacks among
/// `losses`.
pub(crate) fn suggestions(losses: &[Loss], target: &BlockCatalog) -> Suggested {
    losses
        .iter()
        .filter(|loss| loss.kind == MISSING_BLOCK)
        .filter_map(|loss| {
            let proposal = lumilio_schematic::suggest::suggest(&loss.path, target.ids())?;
            Some((loss.path.clone(), proposal))
        })
        .collect()
}

pub(crate) enum Planned {
    Nothing,
    /// Waiting for the target's block states (a backward conversion).
    Checking(String),
    Working,
    Ready {
        target: String,
        conversion: Conversion,
        bytes: Arc<Vec<u8>>,
        /// Why the block check did not run, when it did not.
        unchecked: Option<String>,
        /// A replacement the target has for each block it lacks (T37).
        suggested: Suggested,
    },
    Failed(String),
}

pub(crate) struct ConvertDialog {
    studio: WeakEntity<StudioView>,
    status: Entity<Submission>,
    file: String,
    bytes: Arc<Vec<u8>>,
    /// The file's data version now.
    current: i32,
    versions: Vec<(String, u32)>,
    picker: Entity<VersionPicker>,
    pub(crate) planned: Planned,
    generation: u64,
    losses_scroll: ScrollHandle,
    /// The version chosen first: the game's (T23).
    preferred: Option<String>,
    /// Every file the studio lists, so a new name is one no file has.
    taken: std::collections::BTreeSet<String>,
    /// Replace the original instead of saving beside it.
    pub(crate) overwrite: bool,
    _subscription: Subscription,
}

/// The file a conversion to `target` is saved as beside `file`:
/// `name (target).litematic` in the same folder, numbered when taken.
pub(crate) fn beside(
    file: &str,
    target: &str,
    taken: &std::collections::BTreeSet<String>,
) -> String {
    let folder = file.rsplit_once('/').map_or("", |(folder, _)| folder);
    let stem = tree::stem(file);
    let at = |name: String| {
        if folder.is_empty() {
            name
        } else {
            format!("{folder}/{name}")
        }
    };
    let first = at(format!("{stem} ({target}).litematic"));
    if !taken.contains(&first) {
        return first;
    }
    (2..)
        .map(|n| at(format!("{stem} ({target}) {n}.litematic")))
        .find(|name| !taken.contains(name))
        .expect("some number is free")
}

impl ConvertDialog {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        studio: WeakEntity<StudioView>,
        file: String,
        bytes: Arc<Vec<u8>>,
        current: i32,
        taken: std::collections::BTreeSet<String>,
        preferred: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let picker = cx.new(|cx| {
            VersionPicker::new(
                "studio-convert-version",
                false,
                tr!("studio-convert-placeholder"),
                window,
                cx,
            )
            .menu_width(px(432.))
        });
        let subscription = cx.subscribe(&picker, |this, _, event: &PickerEvent, cx| match event {
            PickerEvent::Chosen(id) => this.choose(id.clone(), cx),
            PickerEvent::Retry => {
                let _ = this.studio.update(cx, |studio, cx| {
                    studio.want_versions();
                    cx.notify();
                });
                this.picker.update(cx, |picker, cx| {
                    picker.set_choices(Choices::Loading, None, cx)
                });
            }
        });
        Self {
            studio,
            status: cx.new(|_| Submission::default()),
            file,
            bytes,
            current,
            versions: Vec::new(),
            picker,
            planned: Planned::Nothing,
            generation: 0,
            losses_scroll: ScrollHandle::new(),
            preferred,
            taken,
            overwrite: false,
            _subscription: subscription,
        }
    }

    /// The version the dialog converts to, once one is chosen.
    #[cfg(test)]
    pub(crate) fn planned_target(&self) -> Option<&str> {
        match &self.planned {
            Planned::Checking(target) | Planned::Ready { target, .. } => Some(target),
            _ => None,
        }
    }

    /// Where the conversion is written: beside the original, or over it.
    pub(crate) fn destination(&self) -> Option<String> {
        let Planned::Ready { target, .. } = &self.planned else {
            return None;
        };
        (!self.overwrite).then(|| beside(&self.file, target, &self.taken))
    }

    /// The game version, when it arrives after the dialog opened: the next
    /// [`Self::set_versions`] chooses it if nothing is chosen yet.
    pub(crate) fn prefer(&mut self, id: &str) {
        if self.preferred.is_none() {
            self.preferred = Some(id.to_owned());
        }
    }

    /// The studio's version list, as it stands.
    pub(crate) fn set_versions(
        &mut self,
        versions: &Loaded<Vec<(String, u32)>>,
        cx: &mut Context<Self>,
    ) {
        let current = self.current;
        let choices = match versions {
            Loaded::Loading => Choices::Loading,
            Loaded::Failed(_) => Choices::Failed(tr!("studio-convert-versions-failed").to_owned()),
            Loaded::Ready(list) => {
                self.versions = list.clone();
                Choices::Ready(
                    list.iter()
                        .map(|(id, data_version)| Choice {
                            id: id.clone(),
                            primary: true,
                            tag: (i64::from(*data_version) == i64::from(current))
                                .then(|| tr!("studio-convert-current")),
                        })
                        .collect(),
                )
            }
        };
        // The game version first, when the list has it and nothing else
        // was chosen.
        let preferred = self
            .preferred
            .clone()
            .filter(|id| self.versions.iter().any(|(version, _)| version == id))
            .filter(|_| matches!(self.planned, Planned::Nothing));
        self.picker.update(cx, |picker, cx| {
            picker.set_choices(choices, preferred.clone(), cx)
        });
        if let Some(id) = preferred {
            self.choose(id, cx);
        }
        cx.notify();
    }

    /// A version was chosen: convert a copy and see what it costs.
    pub(crate) fn choose(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(&(_, data_version)) = self.versions.iter().find(|(version, _)| *version == id)
        else {
            return;
        };
        self.generation += 1;
        self.status.update(cx, |status, cx| status.clear(cx));
        let backward = i32::try_from(data_version)
            .is_ok_and(|target| direction(self.current, target) == Direction::Backward);
        if backward && let Ok(source) = u32::try_from(self.current) {
            self.planned = Planned::Checking(id.clone());
            // The studio may be the one choosing (opening the dialog on the
            // game version), so it is asked once it is free.
            let studio = self.studio.clone();
            cx.defer(move |cx| {
                let _ = studio.update(cx, |studio, _| studio.want_catalogs(id, source));
            });
        } else {
            self.convert(id, data_version, None, None, cx);
        }
        cx.notify();
    }

    /// The target's block states arrived (or could not be had).
    pub(crate) fn catalogs_arrived(
        &mut self,
        target: &str,
        result: Result<(BlockCatalog, Option<BlockCatalog>), String>,
        cx: &mut Context<Self>,
    ) {
        if !matches!(&self.planned, Planned::Checking(waiting) if waiting == target) {
            return;
        }
        let Some(&(_, data_version)) = self.versions.iter().find(|(id, _)| id == target) else {
            return;
        };
        let (catalogs, unchecked) = match result {
            Ok((target, source)) => (Some(Catalogs { target, source }), None),
            Err(detail) => (None, Some(detail)),
        };
        self.convert(target.to_owned(), data_version, catalogs, unchecked, cx);
    }

    /// Converts a copy on the background executor.
    fn convert(
        &mut self,
        id: String,
        data_version: u32,
        catalogs: Option<Catalogs>,
        unchecked: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let generation = self.generation;
        self.planned = Planned::Working;
        let bytes = self.bytes.clone();
        let work = cx.background_executor().spawn(async move {
            let schematic =
                lumilio_schematic::load_litematic(&bytes).map_err(|error| error.to_string())?;
            let target = i32::try_from(data_version).map_err(|error| error.to_string())?;
            let (bytes, conversion) = lumilio_schematic::convert::to_bytes_for_data_version(
                &schematic,
                target,
                catalogs.as_ref(),
            )
            .map_err(|error| error.to_string())?;
            let suggested = catalogs
                .as_ref()
                .map(|catalogs| suggestions(&conversion.losses, &catalogs.target))
                .unwrap_or_default();
            Ok::<_, String>((bytes, conversion, suggested))
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.planned = match result {
                    Ok((bytes, conversion, suggested)) => Planned::Ready {
                        target: id,
                        conversion,
                        bytes: Arc::new(bytes),
                        unchecked,
                        suggested,
                    },
                    Err(detail) => Planned::Failed(detail),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// The name of the file's version: the bundle's id when it has one.
    fn current_name(&self) -> String {
        self.versions
            .iter()
            .find(|(_, data_version)| i64::from(*data_version) == i64::from(self.current))
            .map_or_else(
                || tr!("studio-convert-data-version", version = self.current),
                |(id, _)| id.clone(),
            )
    }

    fn losses(
        &self,
        target: &str,
        losses: &[Loss],
        suggested: &Suggested,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut column = v_flex().w_full().gap_3();
        for group in Group::ALL {
            let group_losses: Vec<&Loss> = losses
                .iter()
                .filter(|loss| Group::of(loss) == group)
                .collect();
            if group_losses.is_empty() {
                continue;
            }
            let (key, heading) = (group.key(), group.heading());
            let group = group_losses;
            let mut rows: Vec<AnyElement> = group
                .iter()
                .take(LISTED)
                .enumerate()
                .map(|(index, loss)| {
                    v_flex()
                        .debug_selector(move || format!("studio-loss-{key}-{index}"))
                        .w_full()
                        .py(px(4.))
                        .gap(px(2.))
                        .child(
                            div()
                                .font_family(theme::mono_font())
                                .text_xs()
                                .text_color(colors.foreground)
                                .child(loss.path.clone()),
                        )
                        .child({
                            let line = div()
                                .id(SharedString::from(format!("studio-loss-{key}-{index}-why")))
                                .debug_selector(move || format!("studio-loss-{key}-{index}-why"))
                                .text_xs()
                                .text_color(colors.muted)
                                .child(detail(loss, target));
                            match upstream_detail(loss) {
                                Some(upstream) => line.tooltip(move |window, cx| {
                                    gpui_component::tooltip::Tooltip::new(upstream.clone())
                                        .build(window, cx)
                                }),
                                None => line,
                            }
                        })
                        // ia[plugin.litematica]: 看保留方块的替代建议 | 转换对话框「保留」组每行下面的「建议替换为 …」 | 按目标版本的方块表猜一个替代：同形状后缀（_stairs、_slab、_wall…）里材料最接近的、去掉 deepslate_ 等前缀的同名方块、优先原版 minecraft:；猜不到就不写 | 点「为这些方块选替代…」时这些建议已填进映射表，可以照用或改（T37）
                        .children(suggested.get(&loss.path).map(|proposal| {
                            div()
                                .debug_selector(move || {
                                    format!("studio-loss-{key}-{index}-suggested")
                                })
                                .text_xs()
                                .text_color(colors.muted)
                                .child(tr!("studio-convert-suggested", block = proposal.as_str()))
                        }))
                        .into_any_element()
                })
                .collect();
            if group.len() > LISTED {
                rows.push(
                    note(
                        tr!("studio-convert-more", count = group.len() - LISTED),
                        colors,
                    )
                    .into_any_element(),
                );
            }
            let kept = key == "kept";
            let section = kit::section(
                format!("{heading} · {}", group.len()),
                colors,
                kit::list(rows, colors),
            );
            if !kept {
                column = column.child(section);
                continue;
            }
            // ia[plugin.litematica]: 为保留的方块选替代 | 转换对话框「保留」组旁的「为这些方块选替代…」 | 关掉转换对话框，打开替换方块的映射表，每种保留的方块一行，等着选目标 | 目标版本不认识、原样写出的方块（T23、T24）
            let ids: Vec<String> = group.iter().map(|loss| loss.path.clone()).collect();
            let proposals = suggested.clone();
            let studio = self.studio.clone();
            let choose = Key::new("studio-convert-replace-kept")
                .label(tr!("studio-convert-replace-kept"))
                .ghost()
                .small()
                .debug_selector(|| "studio-convert-replace-kept".into())
                .on_click(move |_, window, cx| {
                    let _ = studio.update(cx, |studio, cx| {
                        studio.replace_kept(ids.clone(), proposals.clone(), window, cx);
                    });
                });
            column = column.child(
                v_flex()
                    .w_full()
                    .gap_2()
                    .child(section)
                    .child(note(
                        tr!("studio-convert-kept-note", version = target),
                        colors,
                    ))
                    .child(div().child(choose)),
            );
        }
        let _ = cx;
        kit::keep_wheel(
            div()
                .id("studio-convert-losses")
                .w_full()
                .max_h(px(180.))
                .overflow_y_scroll()
                .track_scroll(&self.losses_scroll)
                .child(column),
            &self.losses_scroll,
        )
        .into_any_element()
    }
}

impl DialogForm for ConvertDialog {
    fn status(&self) -> &Entity<Submission> {
        &self.status
    }

    fn ready(&self, _: &App) -> bool {
        matches!(&self.planned, Planned::Ready { conversion, .. }
            if conversion.direction != Direction::Same)
    }

    fn commit_label(&self) -> SharedString {
        match &self.planned {
            Planned::Ready { conversion, .. } if !conversion.is_lossless() => {
                tr!("studio-convert-anyway").into()
            }
            _ => tr!("studio-convert-commit").into(),
        }
    }

    fn commit(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let Planned::Ready {
            target,
            conversion,
            bytes,
            ..
        } = &self.planned
        else {
            return;
        };
        let (bytes, losses) = (bytes.clone(), conversion.losses.len() as u64);
        let (from, to) = (self.current_name(), target.clone());
        let destination = self.destination();
        let name = destination
            .as_deref()
            .map_or_else(|| tree::stem(&self.file), tree::file_name)
            .to_owned();
        let saved_as = destination.is_some();
        let status = self.status.clone();
        let edit = move |_: &[u8]| {
            Ok(Edited {
                bytes: bytes.as_ref().clone(),
                count: losses,
            })
        };
        let describe = Box::new(move |losses: u64| {
            (
                SchematicOperation {
                    kind: "convert".into(),
                    summary: tr!(
                        "studio-convert-summary",
                        from = from.as_str(),
                        to = to.as_str(),
                        count = losses
                    ),
                },
                if saved_as {
                    tr!(
                        "studio-converted-as",
                        file = name.as_str(),
                        version = to.as_str()
                    )
                } else {
                    tr!(
                        "studio-converted",
                        file = name.as_str(),
                        version = to.as_str()
                    )
                },
            )
        });
        let _ = self.studio.update(cx, |studio, cx| {
            studio.submit_to(status, destination, edit, describe, cx);
        });
    }
}

impl Render for ConvertDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = ShellColors::current(cx);
        let mut column = v_flex()
            .w_full()
            .gap_4()
            .child(note(
                tr!("studio-convert-from", version = self.current_name()),
                colors,
            ))
            .child(field(
                tr!("studio-convert-target"),
                div()
                    .w_full()
                    .debug_selector(|| "studio-convert-picker".into())
                    .child(self.picker.clone()),
                colors,
            ));
        let body: AnyElement = match &self.planned {
            Planned::Nothing => note(tr!("studio-convert-choose"), colors).into_any_element(),
            Planned::Checking(_) | Planned::Working => {
                note(tr!("studio-convert-checking"), colors).into_any_element()
            }
            Planned::Failed(detail) => v_flex()
                .gap_1()
                .child(note(tr!("studio-convert-failed"), colors))
                .child(kit::technical("studio-convert-technical", detail.clone()))
                .into_any_element(),
            Planned::Ready {
                target,
                conversion,
                unchecked,
                suggested,
                ..
            } => match conversion.direction {
                Direction::Same => note(tr!("studio-convert-same"), colors).into_any_element(),
                Direction::Forward => {
                    note(tr!("studio-convert-forward"), colors).into_any_element()
                }
                Direction::Backward if conversion.is_lossless() => v_flex()
                    .w_full()
                    .gap_3()
                    .child(note(tr!("studio-convert-backward-clean"), colors))
                    .children(unchecked_note(target, unchecked.as_deref(), colors))
                    .into_any_element(),
                Direction::Backward => v_flex()
                    .w_full()
                    .gap_3()
                    .child(
                        div()
                            .debug_selector(|| "studio-convert-lossy".into())
                            .text_sm()
                            .text_color(colors.foreground)
                            .child(tr!(
                                "studio-convert-lossy",
                                version = target.as_str(),
                                count = conversion.losses.len()
                            )),
                    )
                    .child(self.losses(target, &conversion.losses, suggested, colors, cx))
                    .children(unchecked_note(target, unchecked.as_deref(), colors))
                    .into_any_element(),
            },
        };
        column = column.child(body);
        if let Planned::Ready { conversion, .. } = &self.planned
            && conversion.direction != Direction::Same
        {
            // ia[plugin.litematica]: 另存或覆盖 | 转换对话框底部「覆盖原文件」开关（默认关） | 关着时写「另存为「名 (y).litematic」，原文件不动」，确认后在同一文件夹写新文件并选中它；打开才覆盖原文件（经备份） | 重名时加编号；新文件也记进历史，恢复那一行就把它收进备份（T23）
            let this = cx.weak_entity();
            let overwrite = Fader::new(
                "studio-convert-overwrite",
                self.overwrite,
                tr!("studio-convert-overwrite"),
                move |_, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.overwrite = !this.overwrite;
                        cx.notify();
                    });
                },
            )
            .shown();
            let saved_as = self.destination().map(|file| {
                note(
                    tr!("studio-convert-save-as", file = tree::file_name(&file)),
                    colors,
                )
                .debug_selector(|| "studio-convert-save-as".into())
            });
            column = column.child(
                v_flex().w_full().gap_2().children(saved_as).child(
                    div()
                        .debug_selector(|| "studio-convert-overwrite".into())
                        .child(overwrite),
                ),
            );
        }
        column
    }
}

/// The three kinds of change a conversion lists (T23): what cannot be got
/// back, what is replaced by a near value, and blocks the target does not
/// know, written as they are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Group {
    Lost,
    Approximated,
    Kept,
}

impl Group {
    const ALL: [Self; 3] = [Self::Lost, Self::Approximated, Self::Kept];

    pub(crate) fn of(loss: &Loss) -> Self {
        match (loss.kind, loss.severity) {
            (MISSING_BLOCK, _) => Self::Kept,
            (_, Severity::Lost) => Self::Lost,
            (_, Severity::Approximated) => Self::Approximated,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Lost => "lost",
            Self::Approximated => "approximated",
            Self::Kept => "kept",
        }
    }

    fn heading(self) -> &'static str {
        match self {
            Self::Lost => tr!("studio-convert-lost"),
            Self::Approximated => tr!("studio-convert-approximated"),
            Self::Kept => tr!("studio-convert-kept"),
        }
    }
}

/// A loss's explanation in the launcher's words: the block check's own
/// kinds, and one sentence for each of Nucleation's stable loss kinds (its
/// own English detail stays on the line's tooltip).
pub(crate) fn detail(loss: &Loss, target: &str) -> String {
    match loss.kind {
        MISSING_BLOCK => tr!(
            "studio-convert-missing-block",
            version = target,
            count = loss.count
        ),
        UNKNOWN_PROPERTY => tr!(
            "studio-convert-unknown-property",
            version = target,
            count = loss.count
        ),
        "flattening_ambiguous" => tr!("studio-convert-loss-flattening-ambiguous").to_owned(),
        "flattening_unknown_block" => tr!(
            "studio-convert-loss-flattening-unknown-block",
            version = target
        ),
        "item_flattening_damage" => tr!("studio-convert-loss-item-damage").to_owned(),
        "component_dropped" => tr!("studio-convert-loss-component-dropped", version = target),
        "entity_merge_ambiguous" => tr!("studio-convert-loss-merge-ambiguous").to_owned(),
        "rename_ambiguous" => tr!("studio-convert-loss-rename-ambiguous").to_owned(),
        "fingerprint_collapse" => tr!("studio-convert-loss-fingerprint").to_owned(),
        "unsupported_in_target" => tr!("studio-convert-loss-unsupported", version = target),
        _ => tr!("studio-convert-loss-other", version = target),
    }
}

/// Nucleation's own words for a loss, kept for the curious: none for the
/// block check's kinds, which say everything already.
fn upstream_detail(loss: &Loss) -> Option<String> {
    (!matches!(loss.kind, MISSING_BLOCK | UNKNOWN_PROPERTY) && !loss.detail.is_empty())
        .then(|| loss.detail.clone())
}

/// Says the block check did not run, with why.
fn unchecked_note(
    target: &str,
    unchecked: Option<&str>,
    colors: ShellColors,
) -> Option<AnyElement> {
    let detail = unchecked?;
    Some(
        v_flex()
            .debug_selector(|| "studio-convert-unchecked".into())
            .gap_1()
            .child(note(
                tr!("studio-convert-unchecked", version = target),
                colors,
            ))
            .child(kit::technical(
                "studio-convert-unchecked-technical",
                detail.to_owned(),
            ))
            .into_any_element(),
    )
}
