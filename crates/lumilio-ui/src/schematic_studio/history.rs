//! 历史 (plan schematic-studio, T12, T40): the operation log and the backups
//! folder in one panel. Each log line says whether its copy is still kept
//! and how large it is, and restores from it; copies no line names (a log
//! that was lost or cut short) are listed below and restore too. The panel
//! shows every file's history, or the open file's.

use gpui::prelude::*;
use gpui::{AnyElement, Context, div, px};
use gpui_component::{Sizable as _, WindowExt as _, h_flex, v_flex};
use lumilio_core::{KeptBackup, SchematicLogEntry};

use super::inspector::operation_label;
use super::{Command, Loaded, StudioView, muted_line, tree};
use crate::controls::Segments;
use crate::key::Key;
use crate::kit;
use crate::theme::{self, ShellColors};
use crate::tr;

pub(super) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// `bytes` in the largest unit that keeps a whole number before the point.
pub(super) fn size_words(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024. && unit < UNITS.len() - 1 {
        value /= 1024.;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// The copies no line of `entries` names.
pub(super) fn loose<'a>(
    entries: &[SchematicLogEntry],
    backups: &'a [KeptBackup],
) -> Vec<&'a KeptBackup> {
    backups
        .iter()
        .filter(|kept| {
            !entries
                .iter()
                .any(|entry| entry.backup.as_deref() == Some(kept.path.as_str()))
        })
        .collect()
}

/// What to put back: a log line's copy, or a copy no line names.
#[derive(Clone, Debug)]
enum Restoring {
    Line { index: usize, made: bool },
    Loose { backup: String },
}

impl StudioView {
    /// Asks for the log and the backups folder together: the panel matches
    /// one to the other.
    pub(super) fn want_history(&self) {
        self.send(Command::History);
        self.send(Command::Backups);
    }

    pub(super) fn history_tab(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        let entries = match &self.history {
            Loaded::Loading => {
                return muted_line(tr!("studio-history-loading"), colors).into_any_element();
            }
            Loaded::Failed(detail) => {
                return v_flex()
                    .gap_2()
                    .child(muted_line(tr!("studio-history-failed"), colors))
                    .child(kit::technical("studio-history-technical", detail.clone()))
                    .into_any_element();
            }
            Loaded::Ready(entries) => entries,
        };
        let backups: &[KeptBackup] = match &self.backups {
            Loaded::Ready(backups) => backups,
            _ => &[],
        };
        let loose = loose(entries, backups);
        if entries.is_empty() && loose.is_empty() {
            return v_flex()
                .debug_selector(|| "studio-history-empty".into())
                .child(muted_line(tr!("studio-history-empty"), colors))
                .into_any_element();
        }
        // Every file's lines, or the open file's when 这份投影 is chosen.
        let only = self.selected.as_deref().filter(|_| self.history_this_only);
        let shown: Vec<&SchematicLogEntry> = entries
            .iter()
            .filter(|entry| only.is_none_or(|file| entry.file == file))
            .collect();
        let loose: Vec<&KeptBackup> = loose
            .into_iter()
            .filter(|kept| only.is_none_or(|file| kept.file == file))
            .collect();
        let total: u64 = backups.iter().map(|kept| kept.size).sum();
        // ia[plugin.litematica]: 看历史与备份的总数 | 材料面板「历史」顶部一行 | 写操作记录条数、备份份数与备份共占多少空间 | 日志（log.jsonl）与备份文件夹（backups/）都在启动器数据目录，这里合在一处看（T40）
        let summary = div()
            .debug_selector(|| "studio-history-summary".into())
            .font_family(theme::mono_font())
            .text_xs()
            .text_color(colors.muted)
            .child(tr!(
                "studio-history-summary",
                operations = entries.len(),
                copies = backups.len(),
                size = size_words(total)
            ));
        let this = cx.weak_entity();
        // ia[plugin.litematica]: 只看这份投影或全部的历史 | 「历史」顶部分段：这份投影 / 全部 | 默认列这个游戏所有投影的操作与备份（删除和只保留一份的文件不是打开的那份）；选「这份投影」只列打开的投影的；没有打开投影时不出分段 | T40
        let filter = self.selected.is_some().then(|| {
            div()
                .debug_selector(|| "studio-history-filter".into())
                .child(Segments::new(
                    "studio-history-filter",
                    &[tr!("studio-history-this"), tr!("studio-history-all")],
                    usize::from(!self.history_this_only),
                    move |index, _, cx| {
                        let _ = this.update(cx, |this, cx| {
                            this.history_this_only = index == 0;
                            cx.notify();
                        });
                    },
                ))
        });
        let now = now();
        let mut column = v_flex().w_full().gap_3().child(
            h_flex()
                .w_full()
                .gap_2()
                .items_center()
                .justify_between()
                .child(summary)
                .children(filter),
        );
        if shown.is_empty() {
            column = column.child(
                div()
                    .debug_selector(|| "studio-history-none-here".into())
                    .child(muted_line(tr!("studio-history-none-here"), colors)),
            );
        } else {
            let rows: Vec<AnyElement> = shown
                .iter()
                .map(|entry| self.history_row(entry, backups, now, colors, cx))
                .collect();
            column = column.child(kit::list(rows, colors));
        }
        if !loose.is_empty() {
            let rows: Vec<AnyElement> = loose
                .iter()
                .enumerate()
                .map(|(at, kept)| self.loose_row(at, kept, now, colors, cx))
                .collect();
            column = column.child(
                div()
                    .debug_selector(|| "studio-history-loose".into())
                    .w_full()
                    .child(kit::section(
                        format!("{} · {}", tr!("studio-history-loose"), loose.len()),
                        colors,
                        v_flex()
                            .w_full()
                            .gap_1()
                            .child(muted_line(tr!("studio-history-loose-note"), colors))
                            .child(kit::list(rows, colors)),
                    )),
            );
        }
        column.into_any_element()
    }

    fn history_row(
        &self,
        entry: &SchematicLogEntry,
        backups: &[KeptBackup],
        now: u64,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let index = entry.index;
        let blocked = self.write_blocked();
        // A line with no backup made its file (a conversion saved beside
        // its original, T23): restoring it takes the file into the backups.
        let made = entry.backup.is_none();
        let kept = entry
            .backup
            .as_deref()
            .map(|path| backups.iter().find(|kept| kept.path == path));
        // The copy is gone (or the folder could not be read): nothing to
        // put back.
        let gone = matches!(kept, Some(None)) && matches!(self.backups, Loaded::Ready(_));
        let file = tree::stem(&entry.file).to_owned();
        let summary = if entry.summary.is_empty() {
            entry.file.clone()
        } else {
            entry.summary.clone()
        };
        let copy = match kept {
            None => tr!("studio-history-made").to_owned(),
            Some(Some(kept)) => tr!("studio-history-copy", size = size_words(kept.size)),
            Some(None) if gone => tr!("studio-history-copy-gone").to_owned(),
            Some(None) => String::new(),
        };
        // ia[plugin.litematica]: 恢复到操作之前 | 材料面板「历史」每行的 ghost「恢复」 | 确认后把这次操作前的备份放回原处，本身也记一行历史；那次操作新写出的文件（另存的转换）则收进备份；备份已不在时置灰；游戏运行中置灰并说明 | 每行写备份大小或「备份已不在」；备份与日志在启动器数据目录，不在游戏目录（T12、T23、T40）
        let restore = Key::new(("studio-restore", index))
            .label(tr!("studio-restore"))
            .ghost()
            .small()
            .debug_selector(move || format!("studio-restore-{index}"))
            .disabled(blocked.is_some() || self.restoring || gone)
            .when_some(blocked, |key, why| key.tooltip(why))
            .on_click(cx.listener(move |view, _, window, cx| {
                view.ask_restore(file.clone(), Restoring::Line { index, made }, window, cx);
            }));
        h_flex()
            .id(("studio-history", index))
            .w_full()
            .py(px(6.))
            .gap_2()
            .items_center()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(div().text_sm().text_color(colors.foreground).child(format!(
                        "{} · {}",
                        operation_label(&entry.operation),
                        tree::stem(&entry.file)
                    )))
                    .child(div().text_xs().text_color(colors.muted).child(format!(
                        "{} · {}",
                        crate::live::relative_time(entry.time, now),
                        summary
                    )))
                    .children((!copy.is_empty()).then(|| {
                        div()
                            .debug_selector(move || format!("studio-history-copy-{index}"))
                            .font_family(theme::mono_font())
                            .text_xs()
                            .text_color(colors.muted)
                            .child(copy)
                    })),
            )
            .child(div().flex_none().child(restore))
            .into_any_element()
    }

    fn loose_row(
        &self,
        at: usize,
        kept: &KeptBackup,
        now: u64,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let blocked = self.write_blocked();
        let (file, backup) = (tree::stem(&kept.file).to_owned(), kept.path.clone());
        // ia[plugin.litematica]: 恢复日志里没有的备份 | 「历史」底部「日志里没有的备份」每行的 ghost「恢复」 | 备份文件夹里有、日志没记的副本（日志丢了或被截断）也能确认后放回它原来的位置，现在的文件先备份，恢复记一行历史；只能写回插件有权限的位置 | T40
        let restore = Key::new(("studio-restore-loose", at))
            .label(tr!("studio-restore"))
            .ghost()
            .small()
            .debug_selector(move || format!("studio-restore-loose-{at}"))
            .disabled(blocked.is_some() || self.restoring)
            .when_some(blocked, |key, why| key.tooltip(why))
            .on_click(cx.listener(move |view, _, window, cx| {
                let restoring = Restoring::Loose {
                    backup: backup.clone(),
                };
                view.ask_restore(file.clone(), restoring, window, cx);
            }));
        h_flex()
            .id(("studio-history-loose", at))
            .w_full()
            .py(px(6.))
            .gap_2()
            .items_center()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_sm()
                            .text_color(colors.foreground)
                            .child(kept.file.clone()),
                    )
                    .child(div().text_xs().text_color(colors.muted).child(format!(
                        "{} · {}",
                        crate::live::relative_time(kept.time, now),
                        size_words(kept.size)
                    ))),
            )
            .child(div().flex_none().child(restore))
            .into_any_element()
    }

    fn ask_restore(
        &mut self,
        file: String,
        restoring: Restoring,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let studio = cx.entity().downgrade();
        let body = match restoring {
            Restoring::Line { made: true, .. } => tr!("studio-restore-made-body"),
            Restoring::Line { .. } => tr!("studio-restore-body"),
            Restoring::Loose { .. } => tr!("studio-restore-loose-body"),
        };
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (studio, restoring) = (studio.clone(), restoring.clone());
            alert
                .title(tr!("studio-restore-title", file = file.as_str()))
                .description(body)
                .ok_text(tr!("studio-restore"))
                .cancel_text(tr!("common-cancel"))
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    let _ = studio.update(cx, |view, cx| {
                        view.restoring = true;
                        view.send(match &restoring {
                            Restoring::Line { index, .. } => Command::Restore { index: *index },
                            Restoring::Loose { backup } => Command::RestoreBackup {
                                backup: backup.clone(),
                            },
                        });
                        cx.notify();
                    });
                    true
                })
        });
    }
}
