//! The top of a project page: icon, name, tags, the install button and the
//! numbers. After `ProjectPageHeader` and the install button of
//! `pages/project/Index.vue` (GPL-3.0-only; ADR 0022).

use super::{DetailIntent, ProjectDetailView, pill, spec_cell};
use crate::assets::UiIcon;
use crate::key::Key;
use crate::kit;
use crate::live::{count_label, environment_label, parse_rfc3339, relative_time, tag_label};
use crate::pages::live::project_icon;
use crate::theme::ShellColors;
use gpui::prelude::*;
use gpui::{App, ClipboardItem, Context, IntoElement, Window, div, px};
use gpui_component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_component::{Icon, h_flex, v_flex};
use lumilio_core::{Project, ProjectKind, environment};

/// What the install button and its menu entry run.
type Install = std::rc::Rc<dyn Fn(&mut Window, &mut App)>;

/// What the main button does.
enum Primary {
    /// Install (as a new game for a pack).
    Install,
    /// The game has it and a newer version fits.
    Update {
        file_name: String,
        version_id: String,
    },
    /// The game has it: pick another version on the Versions tab.
    SwitchVersion,
    /// The game has it, and this is the Versions tab.
    Installed,
}

impl ProjectDetailView {
    fn primary(&self, project: &Project) -> Primary {
        let Some(have) = self
            .target
            .as_ref()
            .and_then(|_| self.installed.get(&project.id))
        else {
            return Primary::Install;
        };
        match &have.update {
            Some(version) => Primary::Update {
                file_name: have.file_name.clone(),
                version_id: version.clone(),
            },
            None if self.tab == 1 => Primary::Installed,
            None => Primary::SwitchVersion,
        }
    }

    // ia[discover]: 安装整合包 | 详情页主按钮「安装为新游戏」 | 新建游戏，完成后 toast 可“打开”
    // ia[discover]: 详情页安装按钮 | 主按钮：安装 / 安装中 / 更新 / 切换版本（已装且在介绍页）/ 已安装（已装且在版本页） | 目标游戏已有的才出现后几种；「切换版本」带你去版本页
    fn install_label(&self, project: &Project, primary: &Primary) -> String {
        if self.installing {
            return "安装中…".to_owned();
        }
        match (primary, project.kind, &self.target) {
            (Primary::Update { .. }, _, _) => "更新".to_owned(),
            (Primary::SwitchVersion, _, _) => "切换版本".to_owned(),
            (Primary::Installed, _, _) => "已安装".to_owned(),
            (_, ProjectKind::Modpack, _) => "安装为新游戏".to_owned(),
            (_, _, Some(target)) => format!("安装到 {}", target.name),
            (_, _, None) => "安装".to_owned(),
        }
    }

    pub(super) fn header(
        &self,
        project: &Project,
        owner: Option<&str>,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let primary = self.primary(project);
        let can_press = !self.installing
            && !matches!(primary, Primary::Installed)
            && (project.kind == ProjectKind::Modpack
                || self.target.is_some()
                || matches!(primary, Primary::Install));
        let can_press =
            can_press && (project.kind == ProjectKind::Modpack || self.target.is_some());
        let label = self.install_label(project, &primary);
        let install_label = label.clone();
        let handler = self.handler.clone();
        let title = self.title.to_string();
        let view = cx.entity().downgrade();
        let install: Install = {
            let handler = handler.clone();
            let title = title.clone();
            std::rc::Rc::new(move |window: &mut Window, cx: &mut App| match &primary {
                Primary::Install => handler(
                    DetailIntent::Install {
                        version_id: None,
                        title: title.clone(),
                    },
                    window,
                    cx,
                ),
                Primary::Update {
                    file_name,
                    version_id,
                } => handler(
                    DetailIntent::Update {
                        file_name: file_name.clone(),
                        version_id: version_id.clone(),
                        title: title.clone(),
                    },
                    window,
                    cx,
                ),
                Primary::SwitchVersion => {
                    let _ = view.update(cx, |view, cx| view.select_tab(1, cx));
                }
                Primary::Installed => {}
            })
        };
        let environment = environment(project.client_side, project.server_side);
        let page_url = self.url.to_string();
        let copied = cx.entity().downgrade();

        let actions = kit::PageActions::new("detail-actions")
            .primary(
                crate::theme::clickable(
                    Key::new("detail-install")
                        .debug_selector(|| "detail-install".into())
                        .label(label)
                        .icon(Icon::new(
                            if matches!(self.primary(project), Primary::Install) {
                                UiIcon::Download
                            } else {
                                UiIcon::Switch
                            },
                        ))
                        .primary()
                        .loading(self.installing)
                        .disabled(!can_press),
                    can_press,
                )
                .on_click({
                    let install = install.clone();
                    move |_, window, cx| install(window, cx)
                }),
            )
            .more(kit::MenuEntry::new("在 Modrinth 中打开", {
                let url = page_url.clone();
                move |_, cx| cx.open_url(&url)
            }))
            .more(kit::MenuEntry::new("复制链接", {
                let page_url = page_url.clone();
                let copied = copied.clone();
                move |_, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(page_url.clone()));
                    let _ = copied.update(cx, |view, cx| {
                        view.toast(crate::toast::Toast::success("链接已复制"), cx)
                    });
                }
            }));

        let body = colors.body;
        let updated =
            parse_rfc3339(&project.updated).map(|at| relative_time(at, lumilio_core::unix_now()));
        let header =
            v_flex()
                .w_full()
                .gap_5()
                .child(
                    h_flex()
                        .w_full()
                        .gap_5()
                        .items_start()
                        .child(
                            div()
                                .flex_none()
                                .p(px(4.))
                                .rounded(px(6.))
                                .border_1()
                                .border_color(colors.border)
                                .bg(body.display)
                                .shadow(crate::theme::display_shadow())
                                .child(project_icon(
                                    project.icon_url.as_deref(),
                                    crate::live::seed_of(&project.id),
                                    112.,
                                    colors,
                                )),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .text_size(px(36.))
                                        .line_height(px(42.))
                                        .font_weight(gpui::FontWeight::LIGHT)
                                        .text_color(colors.foreground)
                                        .child(project.title.clone()),
                                )
                                .children(owner.map(|owner| {
                                    div()
                                        .font_family(crate::theme::MONO_FONT)
                                        .text_size(px(11.))
                                        .text_color(colors.muted)
                                        .child(owner.to_uppercase())
                                }))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(colors.muted)
                                        .child(project.description.clone()),
                                )
                                .child(
                                    h_flex()
                                        .flex_wrap()
                                        .gap(px(6.))
                                        .pt(px(2.))
                                        .children(project.loaders.iter().map(|l| {
                                            kit::tag(tag_label(l), kit::TagKind::Ink, colors)
                                        }))
                                        .children(
                                            environment.map(|e| pill(environment_label(e), colors)),
                                        )
                                        .children(
                                            project
                                                .categories
                                                .iter()
                                                .map(|c| pill(tag_label(c), colors)),
                                        ),
                                ),
                        )
                        .children(actions.render(colors)),
                )
                // The datasheet strip: an ink rule over a row of labelled values.
                .child(
                    h_flex()
                        .w_full()
                        .flex_wrap()
                        .gap_y_3()
                        .py(px(12.))
                        .border_t_1()
                        .border_color(colors.foreground)
                        .child(spec_cell("DL", count_label(project.downloads), colors))
                        .child(spec_cell("FAV", count_label(project.followers), colors))
                        .children(updated.map(|updated| spec_cell("UPD", updated, colors)))
                        .children(
                            project
                                .license
                                .clone()
                                .map(|license| spec_cell("LICENSE", license, colors)),
                        ),
                );
        // ia[discover]: 详情页右键菜单 | 在页头上右键：安装 / 在 Modrinth 中打开 / 复制链接 | 与「⋯」菜单同效
        let (open_url, copy_url) = (page_url.clone(), page_url);
        div()
            .id("detail-header")
            .w_full()
            .child(header)
            .context_menu(move |menu, _, _| {
                let (open_url, copy_url) = (open_url.clone(), copy_url.clone());
                let (install, copied) = (install.clone(), copied.clone());
                menu.item(
                    PopupMenuItem::new(install_label.clone())
                        .disabled(!can_press)
                        .on_click(move |_, window, cx| install(window, cx)),
                )
                .separator()
                .item(
                    PopupMenuItem::new("在 Modrinth 中打开").on_click(move |_, _, cx| {
                        cx.open_url(&open_url);
                    }),
                )
                .item(PopupMenuItem::new("复制链接").on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(copy_url.clone()));
                    let _ = copied.update(cx, |view, cx| {
                        view.toast(crate::toast::Toast::success("链接已复制"), cx)
                    });
                }))
            })
    }
}
