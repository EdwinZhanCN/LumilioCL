//! The 资源来源 list under 信息 › 技术信息 (plan schematic-studio, T17, T21,
//! D23, D27): the sources the viewport's blocks are drawn with — the
//! bundle's base version, then each mod and resource pack that holds assets
//! and each pack added from elsewhere — with how many blocks each provides
//! and how many of its models a custom loader draws. Every mod and pack has
//! a switch and is off until switched on for this folder. 添加资源包… only
//! registers a zip's path for the instance.

use gpui::prelude::*;
use gpui::{AnyElement, Context, SharedString, Window, div};
use gpui_component::{Sizable as _, h_flex, v_flex};
use lumilio_core::{ProvidedSource, SourceRole};

use super::inspector::line;
use super::{Command, Placement, StudioView, muted_line, viewport};
use crate::controls::Fader;
use crate::key::Key;
use crate::kit;
use crate::model_view::{base_label, file_label};
use crate::theme::ShellColors;
use crate::toast::Toast;
use crate::tr;

/// The row's label and its trailing count.
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

/// The file a row's switch turns on or off: every mod and pack that is
/// there; not the base, nor an added pack whose file is gone.
fn switchable(source: &ProvidedSource) -> Option<&std::path::Path> {
    match &source.role {
        SourceRole::Added { missing: true, .. } => None,
        role => role.path(),
    }
}

impl StudioView {
    pub(super) fn sources_section(
        &self,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // ia[plugin.litematica]: 添加资源包… | 材料面板「信息」› 技术信息 ›「资源来源」顶部 ghost 键 → 系统文件选择器 | 只登记这个 zip 的路径（记在启动器数据里，不复制文件），在这个文件夹里即打开，叠在打开的 Mod 与资源包之上；视口按新来源重新加载；不是 zip 时提示没有添加 | T17、T21
        let add = Key::new("studio-sources-add")
            .label(tr!("studio-sources-add"))
            .ghost()
            .small()
            .debug_selector(|| "studio-sources-add".into())
            .on_click(cx.listener(|view, _, window, cx| view.pick_pack(window, cx)));
        let body = match (&self.sources, &self.viewer) {
            (Some(sources), _) if !sources.is_empty() => {
                let rows = sources
                    .iter()
                    .enumerate()
                    .map(|(index, source)| self.source_row(index, source, colors, cx))
                    .collect();
                let list = kit::list(rows, colors).into_any_element();
                if sources.iter().any(|source| switchable(source).is_some()) {
                    v_flex()
                        .w_full()
                        .gap_1()
                        .child(list)
                        .child(muted_line(tr!("studio-sources-off"), colors))
                        .into_any_element()
                } else {
                    list
                }
            }
            (_, None) => muted_line(tr!("studio-sources-pick"), colors).into_any_element(),
            (Some(_), Some(_)) => muted_line(tr!("studio-sources-none"), colors).into_any_element(),
            (None, Some(_)) => muted_line(tr!("studio-sources-loading"), colors).into_any_element(),
        };
        // ia[plugin.litematica]: 查看各资源来源提供的方块 | 材料面板「信息」› 技术信息 ›「资源来源」 | 按优先级从低到高列出基础版本（总是下载的 bundle，不烘焙本机游戏）、每个带资源的 Mod 与资源包和各自提供的方块数，以及自定义加载器的模型数（这些模型画不出）；不含资源的 jar 不列出 | 资源包 > Mod > 基础（T17、T21、D23）
        kit::section(
            tr!("studio-sources"),
            colors,
            v_flex()
                .debug_selector(|| "studio-sources".into())
                .w_full()
                .gap_2()
                .child(div().child(add))
                .child(body),
        )
        .into_any_element()
    }

    fn source_row(
        &self,
        index: usize,
        source: &ProvidedSource,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (label, value) = source_words(source);
        let id = SharedString::from(format!("studio-source-{index}"));
        let mut row =
            line(id.clone(), label.clone(), value, colors).debug_selector(move || id.to_string());
        let mut keys = h_flex().flex_none().gap_2().items_center();
        if let Some(path) = switchable(source) {
            let (path, on) = (path.to_owned(), source.enabled);
            let view = cx.weak_entity();
            // ia[plugin.litematica]: 打开或关闭一个 Mod / 资源包 | 「资源来源」每个 Mod 与资源包行尾的开关 | 默认全关；打开后视口叠上它重新加载，关上后不用它重新加载；开关按实例和投影文件夹记在启动器数据里，重开窗口仍在；保存失败时开关退回并提示没有改动 | T21、D23
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
        if switchable(source).is_some() || matches!(source.role, SourceRole::Added { .. }) {
            row = row.child(keys);
        }
        row.into_any_element()
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

    fn pick_pack(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let pick = crate::platform::pick_path(cx, true, false, tr!("studio-sources-prompt"));
        cx.spawn(async move |this, cx| {
            if let Some(path) = pick.await {
                let _ = this.update(cx, |this, _| this.send(Command::AddPack { path }));
            }
        })
        .detach();
    }

    /// A pack was added or taken off: on success the viewport loads again
    /// with the new sources.
    pub(super) fn packs_changed(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        match result {
            Ok(()) => {
                self.redraw_thumbnails();
                // The rows change: the list waits for the new load.
                if self.reload_sources(cx) {
                    self.sources = None;
                }
            }
            Err(detail) => self
                .toasts
                .push(Toast::error(tr!("studio-sources-failed")).technical(detail)),
        }
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
