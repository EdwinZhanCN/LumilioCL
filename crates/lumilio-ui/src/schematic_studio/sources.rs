//! The 资源来源 list under 信息 › 技术信息 (plan schematic-studio, T17, T21,
//! D23, D27): the sources the viewport's blocks are drawn with — the
//! bundle's base version, then each mod and resource pack that holds assets
//! and each pack added from elsewhere — with how many blocks each provides
//! and how many of its models a custom loader draws. Every mod and pack has
//! a switch and is off until switched on for this folder. The list has two
//! parts: what the picture is drawn with (the base and what is switched on,
//! in the order they are layered), then what is not on. 添加… on the
//! section's label registers a zip's or pack folder's path for the instance,
//! picked or typed. 上移 / 下移 change the order the switched-on mods and
//! packs are layered in, kept per instance (T36); the order of those that
//! are off draws nothing, so they have no keys for it.

use gpui::prelude::*;
use gpui::{Anchor, AnyElement, Context, Entity, SharedString, Window, div, px};
use gpui_component::{Sizable as _, h_flex, v_flex};
use lumilio_core::{ProvidedSource, SourceRole};

use super::dialogs::{OpenDialog, PackPathDialog, Submission};
use super::{Command, Placement, StudioView, dialogs, muted_line, viewport};
use crate::controls::Fader;
use crate::key::Key;
use crate::kit::MenuEntry;
use crate::model_view::{base_label, file_label};
use crate::theme::ShellColors;
use crate::toast::Toast;
use crate::{kit, theme, tr};

/// The row's label and its caption: what the source provides.
pub(super) fn source_words(source: &ProvidedSource) -> (String, String) {
    let label = match &source.role {
        // The base is always the bundle's (D23).
        SourceRole::Base { version } => base_label(version),
        SourceRole::Mod(path) | SourceRole::ResourcePack(path) => file_label(path),
        SourceRole::Added { path, .. } => file_label(path),
    };
    let mut value = match &source.role {
        SourceRole::Added { missing: true, .. } => tr!("studio-source-missing").to_owned(),
        _ if source.blocks > 0 => tr!("studio-source-blocks", count = source.blocks),
        _ => tr!("studio-source-files", count = source.assets),
    };
    if source.custom_models > 0 {
        value = format!(
            "{value} · {}",
            tr!("studio-source-custom", count = source.custom_models)
        );
    }
    (label, value)
}

/// The switched-on rows next to one, by the file each is read from.
struct Around {
    /// The row above: lower priority.
    before: Option<std::path::PathBuf>,
    /// The row below: higher priority.
    after: Option<std::path::PathBuf>,
}

/// The file a row's switch turns on or off: every mod and pack that is
/// there; not the base, nor an added pack whose file is gone.
fn switchable(source: &ProvidedSource) -> Option<&std::path::Path> {
    match &source.role {
        SourceRole::Added { missing: true, .. } => None,
        role => role.path(),
    }
}

/// Whether the picture is drawn with the source: the base always, a mod or
/// pack once switched on.
fn in_effect(source: &ProvidedSource) -> bool {
    matches!(source.role, SourceRole::Base { .. })
        || (source.enabled && switchable(source).is_some())
}

impl StudioView {
    /// 添加…'s entries: a zip, a pack folder or a typed path.
    pub(super) fn add_entries(&self, cx: &mut Context<Self>) -> Vec<MenuEntry> {
        let view = cx.weak_entity();
        let pick = move |folder: bool| {
            let view = view.clone();
            move |window: &mut Window, cx: &mut gpui::App| {
                let _ = view.update(cx, |view, cx| view.pick_pack(folder, window, cx));
            }
        };
        let view = cx.weak_entity();
        vec![
            // ia[plugin.litematica]: 添加资源包 | 材料面板「信息」› 技术信息 ›「资源来源」标签右侧白键「添加…」→「资源包（zip）…」→ 系统文件选择器 | 只登记这个 zip 的路径（记在启动器数据里，不复制文件），在这个文件夹里即打开，叠在打开的 Mod 与资源包之上；视口按新来源重新加载；不是 zip 时提示没有添加 | T17、T21
            MenuEntry::new(tr!("studio-sources-add-zip"), pick(false)),
            // ia[plugin.litematica]: 添加资源包文件夹 | 「资源来源」的「添加…」→「资源包文件夹…」→ 系统文件夹选择器 | 解压过的资源包（顶层有 pack.mcmeta 或 assets/）同 zip 一样只登记路径并在这个文件夹里打开；不是资源包时提示没有添加 | 例如红石显示材质包（T36）
            MenuEntry::new(tr!("studio-sources-add-folder"), pick(true)),
            // ia[plugin.litematica]: 按路径添加资源包 | 「资源来源」的「添加…」→「输入路径…」→ 对话框里的路径输入框与「添加」（回车也行） | 粘贴 zip 或资源包文件夹的完整路径后添加，同选择器一样只登记路径；添加后对话框关上；不是资源包时对话框留着，路径保留，下面写没有添加与技术详情 | T36
            MenuEntry::new(tr!("studio-sources-add-path"), move |window, cx| {
                let _ = view.update(cx, |view, cx| view.open_pack_path(window, cx));
            }),
        ]
    }

    pub(super) fn sources_section(
        &self,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let add = kit::with_menu(
            Key::new("studio-sources-add")
                .label(tr!("studio-sources-add"))
                .white()
                .small()
                .debug_selector(|| "studio-sources-add".into()),
            self.add_entries(cx),
            Anchor::TopRight,
            colors,
        );
        let body = match (&self.sources, &self.viewer) {
            (Some(sources), _) if !sources.is_empty() => self.source_lists(sources, colors, cx),
            (_, None) => muted_line(tr!("studio-sources-pick"), colors).into_any_element(),
            (Some(_), Some(_)) => muted_line(tr!("studio-sources-none"), colors).into_any_element(),
            (None, Some(_)) => muted_line(tr!("studio-sources-loading"), colors).into_any_element(),
        };
        // ia[plugin.litematica]: 查看各资源来源提供的方块 | 材料面板「信息」› 技术信息 ›「资源来源」 | 先列画面用到的：基础版本（总是下载的 bundle，不烘焙本机游戏）与打开的 Mod、资源包，按优先级从低到高；再列「未启用」的。每行名字一行写不下就截断（悬停看全名），下面写它提供的方块数与自定义加载器的模型数（这些模型画不出）；不含资源的 jar 不列出 | 资源包 > Mod > 基础（T17、T21、D23）
        kit::section_with(
            tr!("studio-sources"),
            colors,
            add,
            div()
                .debug_selector(|| "studio-sources".into())
                .w_full()
                .child(body),
        )
        .into_any_element()
    }

    /// What the picture is drawn with, then what is not on.
    fn source_lists(
        &self,
        sources: &[ProvidedSource],
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Only the switched-on mods and packs are layered, so only they
        // move, each past its switched-on neighbour.
        let movable: Vec<usize> = sources
            .iter()
            .enumerate()
            .filter(|(_, source)| switchable(source).is_some() && in_effect(source))
            .map(|(index, _)| index)
            .collect();
        let neighbour = |at: Option<usize>| {
            at.and_then(|at| movable.get(at))
                .and_then(|row| switchable(&sources[*row]))
                .map(std::path::Path::to_path_buf)
        };
        let (used, unused): (Vec<usize>, Vec<usize>) =
            (0..sources.len()).partition(|index| in_effect(&sources[*index]));
        let used_rows: Vec<AnyElement> = used
            .iter()
            .map(|index| {
                // One source on alone has no order to change.
                let around = movable
                    .iter()
                    .position(|row| row == index)
                    .filter(|_| movable.len() > 1)
                    .map(|at| Around {
                        before: neighbour(at.checked_sub(1)),
                        after: neighbour(Some(at + 1)),
                    });
                self.source_row(*index, &sources[*index], around, colors, cx)
            })
            .collect();
        let mut column = v_flex()
            .w_full()
            .gap_1()
            .child(kit::list(used_rows, colors))
            .children((movable.len() > 1).then(|| muted_line(tr!("studio-sources-order"), colors)));
        if !unused.is_empty() {
            let rows: Vec<AnyElement> = unused
                .iter()
                .map(|index| self.source_row(*index, &sources[*index], None, colors, cx))
                .collect();
            column = column.child(
                v_flex()
                    .debug_selector(|| "studio-sources-unused".into())
                    .w_full()
                    .pt_3()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(colors.muted)
                            .child(tr!("studio-sources-unused", count = unused.len())),
                    )
                    .child(kit::list(rows, colors))
                    .child(muted_line(tr!("studio-sources-off"), colors)),
            );
        }
        column.into_any_element()
    }

    /// One source: its name on one line (the whole name in the tooltip)
    /// over what it provides, and on the trailing edge 移除 for an added
    /// pack, its switch, and ↑ ↓ when it is on (`around`).
    fn source_row(
        &self,
        index: usize,
        source: &ProvidedSource,
        around: Option<Around>,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (label, value) = source_words(source);
        let id = SharedString::from(format!("studio-source-{index}"));
        let mut keys = h_flex().flex_none().gap_2().items_center();
        if let SourceRole::Added { path, .. } = &source.role {
            let path = path.clone();
            // ia[plugin.litematica]: 移除添加的资源包 | 「资源来源」里添加的资源包行尾 ghost「移除」 | 从来源里去掉、在每个文件夹里关上，并丢弃用它烘焙的缓存，文件本身不动；视口重新加载 | T17、T21
            keys = keys.child(
                Key::new(SharedString::from(format!("studio-source-remove-{index}")))
                    .label(tr!("studio-sources-remove"))
                    .ghost()
                    .small()
                    .debug_selector(move || format!("studio-source-remove-{index}"))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.send(Command::RemovePack { path: path.clone() });
                        cx.notify();
                    })),
            );
        }
        if let Some(path) = switchable(source) {
            let (path, on) = (path.to_owned(), source.enabled);
            let view = cx.weak_entity();
            // ia[plugin.litematica]: 打开或关闭一个 Mod / 资源包 | 「资源来源」每个 Mod 与资源包行尾的开关 | 默认全关；打开后这一行移到上面画面用到的那组，视口叠上它重新加载；关上后移回「未启用」，不用它重新加载；开关按实例和投影文件夹记在启动器数据里，重开窗口仍在；保存失败时开关退回并提示没有改动 | T21、D23
            keys = keys.child(
                div()
                    .debug_selector(move || format!("studio-source-switch-{index}"))
                    .child(Fader::new(
                        SharedString::from(format!("studio-source-switch-{index}")),
                        on,
                        tr!("studio-source-switch", name = label.as_str()),
                        move |_, cx| {
                            let path = path.clone();
                            let _ = view.update(cx, |view, cx| view.switch_source(path, !on, cx));
                        },
                    )),
            );
        }
        if let (Some(path), Some(around)) = (switchable(source), around) {
            // ia[plugin.litematica]: 调整 Mod 与资源包的叠放顺序 | 「资源来源」里打开的每个 Mod 与资源包行尾的「上移」「下移」 | 和上一个或下一个打开的来源换位；排在下面的盖过上面的；顺序按实例记在启动器数据里，换序后视口按新顺序重新烘焙并重画缩略图（旧顺序的烘焙缓存不会被拿来用）；保存失败时提示没有改动 | 关着的来源不参与叠放，没有这两个键；只打开一个时也没有；新装的 Mod 与新启用的资源包排在已排好的上面，新添加的资源包排在最下面（T36）
            for (key, arrow, tip, past) in [
                ("up", "↑", tr!("studio-source-up"), around.before),
                ("down", "↓", tr!("studio-source-down"), around.after),
            ] {
                let path = path.to_owned();
                keys = keys.child(
                    Key::new(SharedString::from(format!("studio-source-{key}-{index}")))
                        .label(arrow)
                        .tooltip(tip)
                        .ghost()
                        .small()
                        .disabled(past.is_none())
                        .debug_selector(move || format!("studio-source-{key}-{index}"))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            if let Some(past) = past.clone() {
                                view.send(Command::MoveSource {
                                    path: path.clone(),
                                    past,
                                });
                                cx.notify();
                            }
                        })),
                );
            }
        }
        let name = SharedString::from(label);
        h_flex()
            .id(id.clone())
            .debug_selector(move || id.to_string())
            .w_full()
            .py(px(5.))
            .gap_2()
            .items_center()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .id(SharedString::from(format!("studio-source-name-{index}")))
                            .debug_selector(move || format!("studio-source-name-{index}"))
                            .w_full()
                            .truncate()
                            .text_sm()
                            .text_color(colors.foreground)
                            .tooltip({
                                let name = name.clone();
                                move |window, cx| {
                                    gpui_component::tooltip::Tooltip::new(name.clone())
                                        .build(window, cx)
                                }
                            })
                            .child(name),
                    )
                    .child(
                        div()
                            .font_family(theme::mono_font())
                            .text_xs()
                            .text_color(colors.muted)
                            .child(value),
                    ),
            )
            .child(keys)
            .into_any_element()
    }

    /// Flips a row's switch at once and asks for it to be kept; the
    /// viewport loads again once it is.
    fn switch_source(&mut self, path: std::path::PathBuf, on: bool, cx: &mut Context<Self>) {
        self.set_enabled(&path, on);
        self.send(Command::SetSource { path, on });
        cx.notify();
    }

    fn set_enabled(&mut self, path: &std::path::Path, on: bool) {
        for source in self.sources.iter_mut().flatten() {
            if source.role.path() == Some(path) {
                source.enabled = on;
            }
        }
    }

    /// Asks the system for a pack zip, or with `folder` a pack folder.
    fn pick_pack(&mut self, folder: bool, _: &mut Window, cx: &mut Context<Self>) {
        let pick = crate::platform::pick_path(cx, !folder, folder, tr!("studio-sources-prompt"));
        cx.spawn(async move |this, cx| {
            if let Some(path) = pick.await {
                let _ = this.update(cx, |this, _| this.send(Command::AddPack { path }));
            }
        })
        .detach();
    }

    /// 输入路径…: the dialog that adds a pack by its typed path.
    pub(super) fn open_pack_path(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let studio = self.this.clone();
        let form = cx.new(|cx| PackPathDialog::new(studio, window, cx));
        self.dialog = Some(OpenDialog::PackPath(form.downgrade()));
        dialogs::show(form, tr!("studio-sources-path-title"), window, cx);
    }

    /// Adds the pack at the path typed into 输入路径…; `status` holds that
    /// dialog open until the answer says whether it was a pack.
    pub(super) fn add_typed(
        &mut self,
        path: std::path::PathBuf,
        status: Entity<Submission>,
        cx: &mut Context<Self>,
    ) {
        status.update(cx, |status, cx| status.start(cx));
        self.pack_adding = Some(status);
        self.send(Command::AddPack { path });
        cx.notify();
    }

    /// The order was kept: the viewport bakes and draws the new stack.
    pub(super) fn source_moved(&mut self, result: Result<bool, String>, cx: &mut Context<Self>) {
        match result {
            Ok(true) => {
                self.redraw_thumbnails();
                // The rows change places: the list waits for the new load.
                if self.reload_sources(cx) {
                    self.sources = None;
                }
            }
            Ok(false) => {}
            Err(detail) => self
                .toasts
                .push(Toast::error(tr!("studio-sources-failed")).technical(detail)),
        }
    }

    /// A pack was added or taken off: on success the viewport loads again
    /// with the new sources. A typed path's answer goes to its dialog: it
    /// closes on success and says why not otherwise.
    pub(super) fn packs_changed(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        let typed = self.pack_adding.take();
        match result {
            Ok(()) => {
                if let Some(status) = typed {
                    status.update(cx, |status, cx| status.finish(cx));
                    self.close_dialog = true;
                }
                self.redraw_thumbnails();
                // The rows change: the list waits for the new load.
                if self.reload_sources(cx) {
                    self.sources = None;
                }
            }
            Err(detail) => match typed {
                Some(status) => status.update(cx, |status, cx| {
                    status.fail(tr!("studio-sources-path-failed"), Some(detail), cx)
                }),
                None => self
                    .toasts
                    .push(Toast::error(tr!("studio-sources-failed")).technical(detail)),
            },
        }
        cx.notify();
    }

    /// A switch was kept: the viewport loads again, the list keeps showing
    /// the switch as flipped until the new load names what is in effect. A
    /// switch that could not be kept flips back.
    pub(super) fn source_switched(
        &mut self,
        path: &std::path::Path,
        on: bool,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(()) => {
                self.redraw_thumbnails();
                self.reload_sources(cx);
            }
            Err(detail) => {
                self.set_enabled(path, !on);
                self.toasts
                    .push(Toast::error(tr!("studio-sources-failed")).technical(detail));
                cx.notify();
            }
        }
    }

    /// Loads the open file's viewport again with the new sources. Returns
    /// whether a file is open.
    fn reload_sources(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(file) = self.selected.clone() else {
            return false;
        };
        // The pane has no viewport: the model is drawn in the window (D22).
        self.viewer = (self.placement == Placement::Window)
            .then(|| viewport::viewer(&file, self.load_model.clone(), cx));
        // The new sources may draw the blocks differently.
        self.old_icons
            .extend(self.icons.drain().map(|(_, icon)| icon));
        self.icons_asked = false;
        self.sync_focus(cx);
        true
    }
}
