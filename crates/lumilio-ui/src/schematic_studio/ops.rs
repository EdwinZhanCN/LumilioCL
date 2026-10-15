//! The workbench's head and page actions (design language §7, D27): the
//! file switcher and the file's size on the leading side; on the trailing
//! edge the one orange key, 替换方块…, and More with every other
//! operation, 删除… last in the danger colour. The operations open their
//! dialogs from here (T13–T16).

use gpui::prelude::*;
use gpui::{AnyElement, Context, Window, div};
use gpui_component::{Sizable as _, h_flex};

use super::dialogs::{self, ConvertDialog, MetadataDialog, OpenDialog, ReplaceDialog};
use super::inspector::size_text;
use super::writes::on_studio;
use super::{Inspection, Loaded, StudioView};
use crate::key::Key;
use crate::kit::{self, MenuEntry};
use crate::theme::{self, ShellColors};
use crate::tr;

/// What the studio hands to the page because the plugin or the system does it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StudioAction {
    /// Select the file in the file manager.
    Reveal(String),
    /// The plugin's material list as CSV, through the host's save dialog.
    ExportCsv(String),
}

impl StudioView {
    pub(super) fn head(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        // The file's name is the switcher's; the size is said once, here.
        // ia[plugin.litematica]: 看投影与游戏的版本 | 工作室头部文件名后的说明 | 写尺寸与「x 投影 · 游戏 y」（两个版本相同只写一个）；游戏版本没有其中某些方块时，后面出橙色标签「y 没有的方块：n 种」与白键「转换到 y…」 | 数据版本按方块资源包索引换成游戏版本名，查不到写数据版本号；比游戏新的投影才在后台按游戏版本的方块表检查（T23）
        let caption = match (&self.selected, &self.inspection) {
            (Some(_), Inspection::Ready(analysis)) => {
                let size = size_text(analysis.stats.size);
                match self.version_words() {
                    Some(words) => format!("{size} · {words}"),
                    None => size,
                }
            }
            (Some(_), _) => String::new(),
            (None, _) => tr!("studio-caption", count = self.tree.files().len()),
        };
        let compat = self.compat_target().map(|target| {
            let count = self.missing_blocks().len();
            let target = target.to_owned();
            let convert_to = target.clone();
            let blocked = self.operation_blocked();
            h_flex()
                .flex_none()
                .gap_2()
                .items_center()
                .child(
                    kit::tag(
                        tr!(
                            "studio-compat-missing",
                            version = target.as_str(),
                            count = count
                        ),
                        kit::TagKind::Orange,
                        colors,
                    )
                    .debug_selector(|| "studio-compat".into()),
                )
                .child(
                    Key::new("studio-compat-convert")
                        .label(tr!("studio-compat-convert", version = target.as_str()))
                        .white()
                        .small()
                        .debug_selector(|| "studio-compat-convert".into())
                        .disabled(blocked.is_some())
                        .when_some(blocked, |key, why| key.tooltip(why))
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.open_convert_to(Some(convert_to.clone()), window, cx);
                        })),
                )
        });
        // ia[plugin.litematica]: 看投影版本未知 | 工作室头部说明后的橙色标签「版本未知」，悬停看说明 | 文件没有记录 MinecraftDataVersion 时不猜版本：说明写「版本未知 · 游戏 y」，按实例的游戏版本取资源绘制，不自动转换，也不和游戏版本比对缺少的方块；「转换版本…」置灰并说明 | 原来静默当作 Nucleation 的规范版本（T39）
        let unknown = self.version_unknown().then(|| {
            div()
                .id("studio-version-unknown")
                .debug_selector(|| "studio-version-unknown".into())
                .flex_none()
                .child(kit::tag(
                    tr!("studio-version-unknown"),
                    kit::TagKind::Orange,
                    colors,
                ))
                .tooltip(|window, cx| {
                    gpui_component::tooltip::Tooltip::new(tr!("studio-version-unknown-why"))
                        .build(window, cx)
                })
        });
        let undrawable = self.undrawable_tag(colors, cx);
        let blocked = self.operation_blocked();
        // ia[plugin.litematica]: 替换方块… | 工作室头部唯一的橙键 → 对话框（源方块与逐个属性的精确值或「任意」、目标方块状态、全部或某个区域） | 改动任一项都在后台重新数出会替换多少个方块，数出来且大于 0 才能确认；确认后在后台替换并经备份写入，记进历史；失败留在对话框里 | 游戏运行中或文件未读完时置灰并说明（T13、D27）
        let replace = Key::new("studio-replace")
            .label(tr!("studio-menu-replace"))
            .primary()
            .debug_selector(|| "studio-replace".into())
            .disabled(blocked.is_some())
            .when_some(blocked, |key, why| key.tooltip(why))
            .on_click(cx.listener(|view, _, window, cx| view.open_replace(window, cx)));
        h_flex()
            .debug_selector(|| "studio-head".into())
            .w_full()
            .items_center()
            .gap_3()
            .child(self.switcher(colors, cx))
            .child(
                div()
                    .debug_selector(|| "studio-caption".into())
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_family(theme::mono_font())
                    .text_xs()
                    .text_color(colors.muted)
                    .child(caption),
            )
            .children(unknown)
            .children(undrawable)
            .children(compat)
            .child(div().flex_none().child(replace))
            .child(
                div()
                    .debug_selector(|| "studio-more".into())
                    .flex_none()
                    .child(kit::more_menu("studio-more", self.menu(), colors)),
            )
            .into_any_element()
    }

    /// 「N 种方块没有模型」 while the picture leaves blocks out (T38): a
    /// click opens 信息 › 技术信息, which lists their IDs.
    fn undrawable_tag(&self, colors: ShellColors, cx: &mut Context<Self>) -> Option<AnyElement> {
        let viewer = self.viewer.as_ref()?.read(cx);
        let count = viewer.undrawable().len();
        if !viewer.is_drawn() || count == 0 {
            return None;
        }
        // ia[plugin.litematica]: 看有多少种方块画不出 | 工作室头部的标签「N 种方块没有模型」 | 视口画完后有画不出的方块才出现；点它切到「信息」并展开技术信息，那里列出这些方块的 ID | 画面里它们是缺失贴图的占位（T38）
        Some(
            div()
                .id("studio-undrawable")
                .debug_selector(|| "studio-undrawable".into())
                .flex_none()
                .cursor_pointer()
                .child(kit::tag(
                    tr!("studio-undrawable-count", count = count),
                    kit::TagKind::Plain,
                    colors,
                ))
                .on_click(cx.listener(|view, _, _, cx| {
                    view.tab = super::InspectorTab::Info;
                    view.technical_open = true;
                    cx.notify();
                }))
                .into_any_element(),
        )
    }

    /// The More menu's entries: every operation but the head's orange
    /// replace, 删除… last (§7, D27).
    pub(super) fn menu(&self) -> Vec<MenuEntry> {
        let chosen = self.selected.clone();
        let pick_first = tr!("studio-pick-first");
        let blocked = self.operation_blocked();
        let write = |label: &'static str, open: fn(&mut Self, &mut Window, &mut Context<Self>)| {
            let entry = MenuEntry::new(label, on_studio(self.this.clone(), open));
            match blocked {
                Some(why) => entry.unavailable(why),
                None => entry,
            }
        };
        let act = self.act.clone();
        let for_file = |label: &'static str, make: fn(String) -> super::StudioAction| {
            let (act, file) = (act.clone(), chosen.clone());
            let entry = MenuEntry::new(label, move |window, cx| {
                if let Some(file) = &file {
                    act(make(file.clone()), window, cx);
                }
            });
            if chosen.is_some() {
                entry
            } else {
                entry.unavailable(pick_first)
            }
        };
        vec![
            // ia[plugin.litematica]: 转换版本… | 工作室更多菜单 → 对话框里的版本选择器（方块资源包索引里的版本，默认选中游戏版本） | 选好版本就在后台试转换：向新版本无损；向旧版本把每条变化按「丢失 / 近似 / 保留」列出；确认后另存为「名 (y).litematic」并选中新文件，打开「覆盖原文件」才写回原文件；取消不写任何东西 | 版本列表读不到时选择器里说明并可重试；游戏运行中置灰（T14、T23）
            if self.version_unknown() {
                MenuEntry::new(
                    tr!("studio-menu-convert"),
                    on_studio(self.this.clone(), Self::open_convert),
                )
                .unavailable(tr!("studio-convert-unknown"))
            } else {
                write(tr!("studio-menu-convert"), Self::open_convert)
            },
            // ia[plugin.litematica]: 导出 PNG… | 工作室更多菜单 → 对话框（宽、高每边最多 4096 像素，背景面板色或透明，角度当前视角或北、东、南、西、顶、等轴） | 在视口的后台线程离屏绘制并编码，弹出保存对话框，选了位置才写入，完成后提示保存位置；预览没准备好或绘制失败时留在对话框里说明 | 切换文件会结束等待中的导出（T10）
            {
                let entry = MenuEntry::new(
                    tr!("studio-menu-png"),
                    on_studio(self.this.clone(), Self::open_export),
                );
                if chosen.is_some() {
                    entry
                } else {
                    entry.unavailable(pick_first)
                }
            },
            write(tr!("studio-menu-metadata"), Self::open_metadata),
            // ia[plugin.litematica]: 导出材料清单（CSV） | 工作室更多菜单 | 插件生成按数量从多到少的 CSV，弹出保存对话框，选了位置才写入；读不了时提示没有导出 | 文件带 UTF-8 标记
            for_file(tr!("studio-menu-csv"), super::StudioAction::ExportCsv),
            // ia[plugin.litematica]: 在访达中显示 | 工作室更多菜单（按系统命名） | 在文件管理器里选中当前投影文件
            for_file(crate::platform::reveal_label(), super::StudioAction::Reveal),
            // ia[plugin.litematica]: 删除… | 工作室更多菜单最后一项（危险色） → 确认框 | 确认后把文件移进启动器的备份（不真正删除），记进历史，可在「历史」里恢复 | 游戏运行中置灰并说明
            write(tr!("studio-menu-delete"), Self::ask_delete).danger(),
        ]
    }

    /// Why an operation on the chosen file cannot start now, if it cannot.
    pub(super) fn operation_blocked(&self) -> Option<&'static str> {
        if self.selected.is_none() {
            Some(tr!("studio-pick-first"))
        } else if let Some(why) = self.write_blocked() {
            Some(why)
        } else if self.opened.is_none() || !matches!(self.inspection, Inspection::Ready(_)) {
            Some(tr!("studio-reading"))
        } else {
            None
        }
    }

    pub(super) fn open_replace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.replace_dialog(window, cx);
    }

    /// The replace dialog on `state`: a bare id (`minecraft:oak_stairs`,
    /// from a material's detail) matches any of its states; a full state
    /// (`minecraft:oak_stairs[facing=east]`) matches each property exactly.
    pub(crate) fn open_replace_for(
        &mut self,
        state: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(form) = self.replace_dialog(window, cx) else {
            return;
        };
        let (id, properties) = super::viewport::split_state(state);
        form.update(cx, |form, cx| form.preset(&id, &properties, window, cx));
    }

    fn replace_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Entity<ReplaceDialog>> {
        let (Some(opened), Inspection::Ready(analysis)) = (&self.opened, &self.inspection) else {
            return None;
        };
        let viewer = self.viewer.as_ref().map(|viewer| viewer.read(cx));
        let opening = dialogs::Opening {
            file: opened.file.clone(),
            bytes: opened.bytes.clone(),
            blocks: analysis.stats.blocks.clone(),
            names: self.names.clone(),
            regions: analysis
                .stats
                .regions
                .iter()
                .map(|region| region.name.clone())
                .collect(),
            selection: viewer.and_then(|viewer| viewer.selection()),
            slice: viewer.and_then(|viewer| viewer.slice_cells()),
            schemes: self.schemes.clone(),
        };
        let studio = self.this.clone();
        let form = cx.new(|cx| ReplaceDialog::new(studio, opening, window, cx));
        self.dialog = Some(OpenDialog::Replace(form.downgrade()));
        dialogs::show(form.clone(), tr!("studio-replace-title"), window, cx);
        self.want_replace_catalog();
        Some(form)
    }

    /// Blocks a conversion keeps as they are (T23), to choose replacements
    /// for: the convert dialog gives way to the replace dialog on them.
    /// `suggested` fills a row's target where it names one (T37).
    pub(crate) fn replace_kept(
        &mut self,
        ids: Vec<String>,
        suggested: dialogs::Suggested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use gpui_component::WindowExt as _;
        window.close_dialog(cx);
        self.dialog = None;
        let Some(form) = self.replace_dialog(window, cx) else {
            return;
        };
        form.update(cx, |form, cx| {
            form.preset_many(&ids, &suggested, window, cx)
        });
    }

    pub(super) fn open_convert(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let game = self.game_version().map(str::to_owned);
        self.open_convert_to(game, window, cx);
    }

    /// The convert dialog, its target `target` (the game version, T23) when
    /// the version list has it; the file is saved beside the original under
    /// a name no file has.
    pub(crate) fn open_convert_to(
        &mut self,
        target: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(opened), Inspection::Ready(analysis)) = (&self.opened, &self.inspection) else {
            return;
        };
        // Converting needs the version the file is at (T39).
        let Some(current) = analysis.info.data_version else {
            return;
        };
        let (file, bytes) = (opened.file.clone(), opened.bytes.clone());
        let studio = self.this.clone();
        let taken: std::collections::BTreeSet<String> = self.tree.files().iter().cloned().collect();
        let form = cx
            .new(|cx| ConvertDialog::new(studio, file, bytes, current, taken, target, window, cx));
        self.want_versions();
        let versions = self.versions.as_ref().unwrap_or(&Loaded::Loading);
        form.update(cx, |form, cx| form.set_versions(versions, cx));
        self.dialog = Some(OpenDialog::Convert(form.downgrade()));
        dialogs::show(form, tr!("studio-convert-title"), window, cx);
    }

    pub(super) fn open_metadata(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(opened), Inspection::Ready(analysis)) = (&self.opened, &self.inspection) else {
            return;
        };
        let file = opened.file.clone();
        let studio = self.this.clone();
        let info = analysis.info.clone();
        let form = cx.new(|cx| MetadataDialog::new(studio, file, &info, window, cx));
        self.dialog = Some(OpenDialog::Metadata(form.downgrade()));
        dialogs::show(form, tr!("studio-meta-title"), window, cx);
    }

    /// The application's review hook: opens `which` (`replace`, `convert`,
    /// or `incompatible` for the replace dialog on the blocks the game
    /// version lacks) once the open file is read; anything else is ignored.
    pub fn review_dialog(&mut self, which: &str, cx: &mut Context<Self>) {
        self.pending_dialog = match which {
            "replace" => Some(ReviewDialog::Replace),
            "convert" => Some(ReviewDialog::Convert),
            "incompatible" => Some(ReviewDialog::Incompatible),
            _ => None,
        };
        cx.notify();
    }

    /// Opens the dialog the review hook asked for once it can.
    pub(super) fn open_review_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(which) = self.pending_dialog else {
            return;
        };
        let read = self.opened.is_some() && matches!(self.inspection, Inspection::Ready(_));
        if !read
            || self.dialog.is_some()
            || (which == ReviewDialog::Incompatible && self.missing_blocks().is_empty())
        {
            return;
        }
        self.pending_dialog = None;
        let target = cx.weak_entity();
        window.defer(cx, move |window, cx| {
            let _ = target.update(cx, |this, cx| match which {
                ReviewDialog::Replace => this.open_replace(window, cx),
                ReviewDialog::Convert => this.open_convert(window, cx),
                ReviewDialog::Incompatible => {
                    let ids = this.missing_blocks().to_vec();
                    let suggested = this.missing_suggested();
                    this.replace_kept(ids, suggested, window, cx);
                }
            });
        });
    }
}

/// The dialogs the review hook can open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReviewDialog {
    Replace,
    Convert,
    Incompatible,
}
