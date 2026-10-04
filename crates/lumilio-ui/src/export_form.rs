//! The export-modpack dialog (IA `instance/README.md`, L-LIB-08): the pack's
//! name, version and summary, and which top-level files and folders of the
//! game go in. It holds no business logic: the answer is an [`ExportSpec`]
//! the application hands to core.

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::controls::Checkbox;
use crate::key::Key;
use gpui::{App, Context, Entity, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{
    ActiveTheme as _, Sizable as _, StyledExt as _, WindowExt as _, h_flex, v_flex,
};
use lumilio_core::{ExportSpec, FileEntry, PackFormat};

use crate::kit;
use crate::theme::{self, ShellColors};

const DIALOG_WIDTH: f32 = 460.;
pub const FORMAT_LABELS: [&str; 2] = ["Modrinth (.mrpack)", "MultiMC / Prism (.zip)"];

/// What goes in unless the person says otherwise. Worlds, logs and crash
/// reports are personal and large; they are never in by default.
pub const DEFAULT_INCLUDE: [&str; 4] = ["mods", "config", "resourcepacks", "shaderpacks"];

pub type ExportHandler = Rc<dyn Fn(ExportSpec, &mut Window, &mut App)>;

/// One top-level entry of the game directory, and — for a folder worth
/// opening — what is directly inside it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Node {
    pub name: String,
    /// The whole entry goes in. For a folder with children: all of them.
    pub on: bool,
    /// Directly inside a folder: name and whether it goes in.
    pub children: Vec<(String, bool)>,
    pub expanded: bool,
}

impl Node {
    /// Ticks or unticks the entry, and everything inside it with it.
    pub fn toggle(&mut self) {
        self.on = !self.on;
        for child in &mut self.children {
            child.1 = self.on;
        }
    }

    /// Ticks or unticks one thing inside; the folder counts as ticked only
    /// when all of it is.
    pub fn toggle_child(&mut self, index: usize) {
        if let Some(child) = self.children.get_mut(index) {
            child.1 = !child.1;
        }
        self.on = !self.children.is_empty() && self.children.iter().all(|(_, on)| *on);
    }

    /// The paths this entry puts in: itself when all of it goes in, else just
    /// the ticked parts.
    #[must_use]
    pub fn paths(&self) -> Vec<String> {
        if self.children.is_empty() {
            return if self.on {
                vec![self.name.clone()]
            } else {
                Vec::new()
            };
        }
        if self.on {
            return vec![self.name.clone()];
        }
        self.children
            .iter()
            .filter(|(_, on)| *on)
            .map(|(child, _)| format!("{}/{child}", self.name))
            .collect()
    }
}

/// Folders whose insides are not offered one by one: they can be huge, and
/// worlds and logs are not meant for a pack anyway.
const NOT_OPENED: [&str; 3] = ["saves", "logs", "crash-reports"];

/// The entries the dialog offers: the top level of the game directory,
/// without the game's own bookkeeping files, with what is inside each
/// folder `below` knows about.
#[must_use]
pub fn tree(entries: &[FileEntry], below: &BTreeMap<String, Vec<FileEntry>>) -> Vec<Node> {
    entries
        .iter()
        .filter(|entry| !entry.name.starts_with('.') && entry.name != "session.lock")
        .map(|entry| {
            let on = DEFAULT_INCLUDE.contains(&entry.name.as_str());
            let children = if entry.is_dir && !NOT_OPENED.contains(&entry.name.as_str()) {
                below
                    .get(&entry.name)
                    .map(|inside| {
                        inside
                            .iter()
                            .filter(|child| !child.name.starts_with('.'))
                            .map(|child| (child.name.clone(), on))
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            Node {
                name: entry.name.clone(),
                on,
                children,
                expanded: false,
            }
        })
        .collect()
}

/// The spec the typed values make, or why not (shown when something was typed).
pub fn spec_from(
    name: &str,
    version: &str,
    summary: &str,
    nodes: &[Node],
    format: PackFormat,
) -> Result<ExportSpec, &'static str> {
    if name.trim().is_empty() {
        return Err("请输入整合包名称");
    }
    if version.trim().is_empty() {
        return Err("请输入版本号");
    }
    let include: Vec<String> = nodes.iter().flat_map(Node::paths).collect();
    if include.is_empty() {
        return Err("至少选一项要放进整合包的内容");
    }
    Ok(ExportSpec {
        format,
        name: name.trim().to_owned(),
        version: version.trim().to_owned(),
        summary: (!summary.trim().is_empty()).then(|| summary.trim().to_owned()),
        include,
    })
}

pub struct ExportForm {
    name: Entity<InputState>,
    version: Entity<InputState>,
    summary: Entity<InputState>,
    nodes: Vec<Node>,
    format: PackFormat,
    handler: ExportHandler,
}

impl ExportForm {
    pub fn new(
        game_name: &str,
        entries: &[FileEntry],
        below: &BTreeMap<String, Vec<FileEntry>>,
        handler: ExportHandler,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let game_name = game_name.to_owned();
        let name = cx.new(|cx| InputState::new(window, cx).default_value(game_name));
        let version = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("例如 1.0.0")
                .default_value("1.0.0")
        });
        let summary = cx.new(|cx| InputState::new(window, cx).placeholder("一句话介绍（可空）"));
        for input in [&name, &version, &summary] {
            cx.subscribe_in(
                input,
                window,
                |form, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Change => cx.notify(),
                    InputEvent::PressEnter { .. } => form.submit(window, cx),
                    _ => {}
                },
            )
            .detach();
        }
        Self {
            name,
            version,
            summary,
            nodes: tree(entries, below),
            format: PackFormat::Modrinth,
            handler,
        }
    }

    fn typed(&self, cx: &App) -> [String; 3] {
        [&self.name, &self.version, &self.summary].map(|input| input.read(cx).value().to_string())
    }

    /// The finished spec, or why it cannot be made yet.
    pub fn spec(&self, cx: &App) -> Result<ExportSpec, &'static str> {
        let [name, version, summary] = self.typed(cx);
        spec_from(&name, &version, &summary, &self.nodes, self.format)
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Ok(spec) = self.spec(cx) {
            (self.handler)(spec, window, cx);
            window.close_dialog(cx);
        }
    }

    pub fn open(form: Entity<Self>, window: &mut Window, cx: &mut App) {
        window.open_dialog(cx, move |dialog, _, cx| {
            let ready = form.read(cx).spec(cx).is_ok();
            let weak = form.downgrade();
            let ok = theme::clickable(
                Key::new("export-ok")
                    .label("选择位置并导出")
                    .primary()
                    .disabled(!ready)
                    .debug_selector(|| "export-ok".into())
                    .on_click(move |_, window, cx| {
                        let _ = weak.update(cx, |form, cx| form.submit(window, cx));
                    }),
                ready,
            );
            theme::dialog(dialog, cx)
                .title("导出整合包")
                .w(px(DIALOG_WIDTH))
                .child(form.clone())
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Key::new("export-cancel")
                                .label("取消")
                                .white()
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(ok),
                )
        });
    }
}

fn label(text: &'static str, colors: ShellColors) -> impl IntoElement {
    div()
        .text_sm()
        .font_medium()
        .text_color(colors.foreground)
        .child(text)
}

impl Render for ExportForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = ShellColors::from_theme(cx.theme());
        let weak = cx.entity().downgrade();
        let [name, version, summary] = self.typed(cx);
        // Nothing is nagged about until something was typed or unticked.
        let problem = spec_from(&name, &version, &summary, &self.nodes, self.format)
            .err()
            .filter(|_| {
                !name.trim().is_empty() || self.nodes.iter().all(|node| node.paths().is_empty())
            });
        v_flex()
            .id("export-form")
            .w_full()
            .gap_3()
            .child(
                v_flex()
                    .gap_1()
                    .child(label("名称", colors))
                    .child(Input::new(&self.name)),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(label("版本号", colors))
                    .child(Input::new(&self.version)),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(label("简介", colors))
                    .child(Input::new(&self.summary)),
            )
            .child(label("格式", colors))
            .child(h_flex().child(kit::segments(
                "export-format",
                &FORMAT_LABELS,
                usize::from(self.format == PackFormat::Prism),
                {
                    let weak = weak.clone();
                    move |index, _, cx| {
                        let _ = weak.update(cx, |form, cx| {
                            form.format = if index == 1 {
                                PackFormat::Prism
                            } else {
                                PackFormat::Modrinth
                            };
                            cx.notify();
                        });
                    }
                },
            )))
            .child(label("放进整合包的内容", colors))
            .child(
                v_flex()
                    .id("export-entries")
                    .max_h(px(220.))
                    .overflow_y_scroll()
                    .gap_2()
                    .children(self.nodes.iter().enumerate().map(|(index, node)| {
                        let (toggle, expand) = (weak.clone(), weak.clone());
                        let head = h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Checkbox::new(("export-entry", index))
                                    .checked(node.on)
                                    .label(node.name.clone())
                                    .on_click(move |_, _, cx| {
                                        let _ = toggle.update(cx, |form, cx| {
                                            if let Some(node) = form.nodes.get_mut(index) {
                                                node.toggle();
                                            }
                                            cx.notify();
                                        });
                                    }),
                            )
                            .children((!node.children.is_empty()).then(|| {
                                Key::new(("export-expand", index))
                                    .label(if node.expanded { "收起" } else { "展开" })
                                    .ghost()
                                    .xsmall()
                                    .debug_selector(move || format!("export-expand-{index}"))
                                    .on_click(move |_, _, cx| {
                                        let _ = expand.update(cx, |form, cx| {
                                            if let Some(node) = form.nodes.get_mut(index) {
                                                node.expanded = !node.expanded;
                                            }
                                            cx.notify();
                                        });
                                    })
                            }));
                        let inside =
                            node.expanded.then(|| {
                                v_flex().pl_6().gap_1().children(
                                    node.children.iter().enumerate().map(|(at, (child, on))| {
                                        let weak = weak.clone();
                                        Checkbox::new(("export-child", index * 10_000 + at))
                                            .checked(*on)
                                            .label(child.clone())
                                            .on_click(move |_, _, cx| {
                                                let _ = weak.update(cx, |form, cx| {
                                                    if let Some(node) = form.nodes.get_mut(index) {
                                                        node.toggle_child(at);
                                                    }
                                                    cx.notify();
                                                });
                                            })
                                    }),
                                )
                            });
                        v_flex().gap_1().child(head).children(inside)
                    })),
            )
            .child(div().text_xs().text_color(colors.muted).child(
                "Modrinth 上有的文件按地址列出，其余的会直接放进整合包。存档和日志默认不放。",
            ))
            .children(problem.map(|problem| {
                div()
                    .text_xs()
                    .text_color(colors.danger)
                    .debug_selector(|| "export-problem".into())
                    .child(problem)
            }))
    }
}

#[cfg(test)]
mod tests;
