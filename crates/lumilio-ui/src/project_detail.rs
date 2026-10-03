//! A project's detail window (Discover): description, versions, gallery, and
//! a "more" menu. Native, rendered from Modrinth's own data; the public web
//! page is only a link away.

use std::rc::Rc;

use crate::key::Key;
use gpui::{
    App, ClipboardItem, Context, Div, IntoElement, ObjectFit, Render, SharedString,
    StyledImage as _, Window, div, img, prelude::*, px,
};
use gpui_component::text::TextView;
use gpui_component::{
    ActiveTheme as _, Icon, Sizable as _, StyledExt as _, TITLE_BAR_HEIGHT, h_flex, v_flex,
};
use lumilio_core::{
    Loader, Project, ProjectDetail, ProjectKind, ReleaseChannel, Version, environment, version_fits,
};

use crate::assets::UiIcon;
use crate::kit;
use crate::live::{count_label, environment_label, parse_rfc3339, relative_time, tag_label};
use crate::pages::live::project_icon;
use crate::theme::ShellColors;

pub const TABS: [&str; 3] = ["介绍", "版本", "画廊"];

/// What the window asks the application to do.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DetailIntent {
    /// Install this version, or the newest that fits when `None`.
    Install {
        version_id: Option<String>,
        title: String,
    },
    /// Download this version's file to a place the person chooses.
    SaveAs {
        version_id: String,
        file_name: String,
        title: String,
    },
    /// Replace the installed file by this newer version.
    Update {
        file_name: String,
        version_id: String,
        title: String,
    },
}

pub type DetailHandler = Rc<dyn Fn(DetailIntent, &mut Window, &mut App)>;

/// The instance installs would go into, so versions that do not fit it can
/// say so.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallTarget {
    pub name: String,
    pub game_version: String,
    pub loader: Loader,
}

#[derive(Clone, Debug)]
pub enum DetailState {
    Loading,
    Failed(String),
    Ready(Box<ProjectDetail>),
}

pub struct ProjectDetailView {
    title: SharedString,
    url: SharedString,
    state: DetailState,
    tab: usize,
    target: Option<InstallTarget>,
    /// What the target game already has, by project.
    installed: std::collections::BTreeMap<String, lumilio_core::InstalledProject>,
    /// Messages waiting for the next render to float up (§11).
    toasts: Vec<crate::toast::Toast>,
    handler: DetailHandler,
    /// How many versions the list shows before "show more".
    versions_shown: usize,
}

const VERSIONS_PAGE: usize = 25;

impl ProjectDetailView {
    pub fn new(
        title: impl Into<SharedString>,
        url: impl Into<SharedString>,
        handler: DetailHandler,
    ) -> Self {
        Self {
            title: title.into(),
            url: url.into(),
            state: DetailState::Loading,
            tab: 0,
            target: None,
            installed: std::collections::BTreeMap::new(),
            toasts: Vec::new(),
            handler,
            versions_shown: VERSIONS_PAGE,
        }
    }

    pub fn set_state(&mut self, state: DetailState, cx: &mut Context<Self>) {
        if let DetailState::Ready(detail) = &state {
            self.title = detail.project.title.clone().into();
        }
        self.state = state;
        cx.notify();
    }

    pub fn set_target(&mut self, target: Option<InstallTarget>, cx: &mut Context<Self>) {
        self.target = target;
        cx.notify();
    }

    pub fn set_installed(
        &mut self,
        installed: std::collections::BTreeMap<String, lumilio_core::InstalledProject>,
        cx: &mut Context<Self>,
    ) {
        self.installed = installed;
        cx.notify();
    }

    /// Sets the target while building, before there is a window to redraw.
    pub fn set_target_quiet(&mut self, target: Option<InstallTarget>) {
        self.target = target;
    }

    pub fn toast(&mut self, toast: crate::toast::Toast, cx: &mut Context<Self>) {
        self.toasts.push(toast);
        cx.notify();
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn state(&self) -> &DetailState {
        &self.state
    }

    pub fn select_tab(&mut self, tab: usize, cx: &mut Context<Self>) {
        self.tab = tab.min(TABS.len() - 1);
        cx.notify();
    }
}

/// `1.4 MB`, `812 KB`, `37 B`.
pub fn size_label(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let value = bytes as f64;
    if value >= KB * KB * KB {
        format!("{:.1} GB", value / (KB * KB * KB))
    } else if value >= KB * KB {
        format!("{:.1} MB", value / (KB * KB))
    } else if value >= KB {
        format!("{:.0} KB", value / KB)
    } else {
        format!("{bytes} B")
    }
}

/// `2024-01-05` from an RFC 3339 time; empty when it is not one.
pub fn date_label(iso: &str) -> String {
    match iso.get(..10) {
        Some(day) if parse_rfc3339(iso).is_some() => day.to_owned(),
        _ => String::new(),
    }
}

const fn channel_label(channel: ReleaseChannel) -> &'static str {
    match channel {
        ReleaseChannel::Release => "正式版",
        ReleaseChannel::Beta => "测试版",
        ReleaseChannel::Alpha => "早期版",
    }
}

/// `1.21, 1.21.1, 1.20.6 +4`.
pub fn versions_label(versions: &[String]) -> String {
    const SHOWN: usize = 3;
    let head = versions
        .iter()
        .take(SHOWN)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    match versions.len().saturating_sub(SHOWN) {
        0 => head,
        more => format!("{head} +{more}"),
    }
}

fn pill(text: impl Into<SharedString>, colors: ShellColors) -> impl IntoElement {
    kit::tag(text, kit::TagKind::Plain, colors)
}

/// One cell of the datasheet strip: a muted mono legend over its value.
fn spec_cell(legend: &'static str, value: impl Into<SharedString>, colors: ShellColors) -> Div {
    v_flex()
        .gap(px(3.))
        .pr(px(28.))
        .child(
            div()
                .font_family(crate::theme::MONO_FONT)
                .text_size(px(10.))
                .text_color(colors.muted)
                .child(legend),
        )
        .child(
            div()
                .font_family(crate::theme::MONO_FONT)
                .text_sm()
                .text_color(colors.foreground)
                .child(value.into()),
        )
}

/// The file a version offers: the primary one, else the first.
fn file_of(version: &Version) -> Option<&lumilio_core::VersionFile> {
    version
        .files
        .iter()
        .find(|file| file.primary)
        .or(version.files.first())
}

impl ProjectDetailView {
    // ia[discover]: 安装整合包 | 详情页主按钮「安装为新游戏」 | 新建游戏，完成后 toast 可“打开” | H-INSTALL-03
    fn install_label(&self, project: &Project) -> String {
        let kind = project.kind;
        match (kind, &self.target) {
            (_, Some(_))
                if self
                    .installed
                    .get(&project.id)
                    .is_some_and(|have| have.update.is_some()) =>
            {
                "更新".to_owned()
            }
            (_, Some(_)) if self.installed.contains_key(&project.id) => "已安装".to_owned(),
            (ProjectKind::Modpack, _) => "安装为新游戏".to_owned(),
            (_, Some(target)) => format!("安装到 {}", target.name),
            (_, None) => "安装".to_owned(),
        }
    }

    fn fits_target(&self, project: &Project, version: &Version) -> bool {
        match (&self.target, project.kind) {
            (_, ProjectKind::Modpack) | (None, _) => true,
            (Some(target), kind) => {
                version_fits(version, kind, &target.game_version, target.loader)
            }
        }
    }

    fn header(
        &self,
        project: &Project,
        owner: Option<&str>,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let have = self.installed.get(&project.id).cloned();
        let install = {
            let handler = self.handler.clone();
            let title = self.title.to_string();
            let have = have.clone();
            move |window: &mut Window, cx: &mut App| {
                let intent = match &have {
                    Some(have) => match &have.update {
                        Some(version) => DetailIntent::Update {
                            file_name: have.file_name.clone(),
                            version_id: version.clone(),
                            title: title.clone(),
                        },
                        None => return,
                    },
                    None => DetailIntent::Install {
                        version_id: None,
                        title: title.clone(),
                    },
                };
                handler(intent, window, cx);
            }
        };
        let up_to_date =
            self.target.is_some() && have.as_ref().is_some_and(|have| have.update.is_none());
        let can_install =
            (project.kind == ProjectKind::Modpack || self.target.is_some()) && !up_to_date;
        let environment = environment(project.client_side, project.server_side);
        let page_url = self.url.to_string();
        let copied = cx.entity().downgrade();

        let mut actions = kit::PageActions::new("detail-actions")
            .primary(
                crate::theme::clickable(
                    Key::new("detail-install")
                        .label(self.install_label(project))
                        .icon(Icon::new(UiIcon::Download))
                        .primary()
                        .disabled(!can_install),
                    can_install,
                )
                .on_click(move |_, window, cx| install(window, cx)),
            )
            .more(kit::MenuEntry::new("在 Modrinth 中打开", {
                let url = page_url.clone();
                move |_, cx| cx.open_url(&url)
            }))
            .more(kit::MenuEntry::new("复制链接", move |_, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(page_url.clone()));
                let _ = copied.update(cx, |view, cx| {
                    view.toast(crate::toast::Toast::success("链接已复制"), cx)
                });
            }));
        for (label, link) in [
            ("源码", &project.links.source),
            ("问题反馈", &project.links.issues),
            ("Wiki", &project.links.wiki),
            ("Discord", &project.links.discord),
        ] {
            if let Some(link) = link.clone() {
                actions = actions.more(kit::MenuEntry::new(label, move |_, cx| cx.open_url(&link)));
            }
        }

        let body = colors.body;
        let updated =
            parse_rfc3339(&project.updated).map(|at| relative_time(at, lumilio_core::unix_now()));
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
                                    .children(
                                        project.loaders.iter().map(|l| {
                                            kit::tag(tag_label(l), kit::TagKind::Ink, colors)
                                        }),
                                    )
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
            )
    }

    fn description(&self, project: &Project, colors: ShellColors) -> gpui::AnyElement {
        let text = if project.body.trim().is_empty() {
            project.description.clone()
        } else {
            project.body.clone()
        };
        div()
            .w_full()
            .debug_selector(|| "detail-description".into())
            .text_color(colors.foreground)
            .child(TextView::markdown("detail-body", text).selectable(true))
            .into_any_element()
    }

    fn versions(
        &self,
        detail: &ProjectDetail,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        if detail.versions.is_empty() {
            return kit::empty("还没有可用的版本", "", colors).into_any_element();
        }
        let project = &detail.project;
        let installable = project.kind != ProjectKind::Modpack;
        let rows = detail
            .versions
            .iter()
            .take(self.versions_shown)
            .enumerate()
            .map(|(index, version)| {
                let fits = self.fits_target(project, version);
                let handler = self.handler.clone();
                let id = version.id.clone();
                let title = self.title.to_string();
                // ia[discover]: 另存为文件 | 详情页版本行「另存为…」 | 选位置下载，任何类型，包括整合包文件 | H-DISC-05
                let save = file_of(version).map(|file| {
                    let (handler, id, title) = (handler.clone(), id.clone(), title.clone());
                    let file_name = file.filename.clone();
                    Key::new(("detail-version-save", index))
                        .label("另存为…")
                        .ghost()
                        .small()
                        .debug_selector(move || format!("detail-version-save-{index}"))
                        .on_click(move |_, window, cx| {
                            handler(
                                DetailIntent::SaveAs {
                                    version_id: id.clone(),
                                    file_name: file_name.clone(),
                                    title: title.clone(),
                                },
                                window,
                                cx,
                            );
                        })
                });
                let file = version
                    .files
                    .iter()
                    .find(|f| f.primary)
                    .or(version.files.first());
                h_flex()
                    .w_full()
                    .gap_4()
                    .py(px(12.))
                    .items_center()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(4.))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .font_semibold()
                                            .text_color(colors.foreground)
                                            .child(version.name.clone()),
                                    )
                                    .child(pill(channel_label(version.channel), colors))
                                    .children((!fits).then(|| pill("不适用于当前游戏", colors))),
                            )
                            .child(div().text_xs().text_color(colors.muted).child(format!(
                                    "{} · {} · {}",
                                    version.number,
                                    versions_label(&version.game_versions),
                                    version
                                        .loaders
                                        .iter()
                                        .map(|l| tag_label(l))
                                        .collect::<Vec<_>>()
                                        .join(" / ")
                                ))),
                    )
                    .child(
                        v_flex()
                            .items_end()
                            .gap(px(2.))
                            .text_xs()
                            .text_color(colors.muted)
                            .child(date_label(&version.published))
                            .child(format!(
                                "{} 次下载{}",
                                count_label(version.downloads),
                                file.map(|f| format!(" · {}", size_label(f.size)))
                                    .unwrap_or_default()
                            )),
                    )
                    // ia[discover]: 安装指定版本 | 详情页版本标签行内「安装」 | 后台任务；与目标游戏不兼容的版本禁用 | H-DISC-03/04
                    .children(installable.then(|| {
                        Key::new(("detail-version-install", index))
                            .label("安装")
                            .icon(Icon::new(UiIcon::Download))
                            .white()
                            .small()
                            .disabled(!fits || self.target.is_none())
                            .on_click(move |_, window, cx| {
                                handler(
                                    DetailIntent::Install {
                                        version_id: Some(id.clone()),
                                        title: title.clone(),
                                    },
                                    window,
                                    cx,
                                );
                            })
                    }))
                    .children(save)
            })
            .collect::<Vec<_>>();
        let more = (detail.versions.len() > self.versions_shown).then(|| {
            let view = cx.entity();
            kit::ghost(
                "detail-more-versions",
                "显示更多版本",
                move |_, cx| {
                    view.update(cx, |view, cx| {
                        view.versions_shown += VERSIONS_PAGE;
                        cx.notify();
                    });
                },
            )
        });
        v_flex()
            .w_full()
            .gap_3()
            .debug_selector(|| "detail-versions".into())
            .child(kit::panel_list(rows, colors))
            .children(more)
            .into_any_element()
    }

    fn gallery(&self, project: &Project, colors: ShellColors) -> gpui::AnyElement {
        if project.gallery.is_empty() {
            return kit::empty("这个项目没有画廊", "", colors).into_any_element();
        }
        h_flex()
            .w_full()
            .flex_wrap()
            .gap_4()
            .debug_selector(|| "detail-gallery".into())
            .children(project.gallery.iter().enumerate().map(|(index, image)| {
                let link = image.url.clone();
                v_flex()
                    .id(("detail-gallery", index))
                    .w(px(440.))
                    .gap_2()
                    .cursor_pointer()
                    .on_click(move |_, _, cx| cx.open_url(&link))
                    .child(
                        div()
                            .w_full()
                            .h(px(248.))
                            .rounded(px(12.))
                            .overflow_hidden()
                            .bg(colors.surface_subtle)
                            .child(
                                img(image.url.clone())
                                    .size_full()
                                    .object_fit(ObjectFit::Cover),
                            ),
                    )
                    .children((!image.title.is_empty()).then(|| {
                        div()
                            .text_sm()
                            .font_medium()
                            .text_color(colors.foreground)
                            .child(image.title.clone())
                    }))
                    .children((!image.description.is_empty()).then(|| {
                        div()
                            .text_xs()
                            .text_color(colors.muted)
                            .child(image.description.clone())
                    }))
            }))
            .into_any_element()
    }
}

impl Render for ProjectDetailView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::toast::flush(&mut self.toasts, window, cx);
        let colors = ShellColors::from_theme(cx.theme());
        let body = match self.state.clone() {
            DetailState::Loading => kit::empty("正在读取…", "", colors).into_any_element(),
            DetailState::Failed(message) => v_flex()
                .items_center()
                .child(kit::empty(
                    "连不上 Modrinth",
                    "检查网络后再打开一次",
                    colors,
                ))
                .child(kit::technical("detail-technical", message))
                .into_any_element(),
            DetailState::Ready(detail) => {
                let counts = [
                    String::new(),
                    format!(" {}", detail.versions.len()),
                    format!(" {}", detail.project.gallery.len()),
                ];
                let labels: Vec<&'static str> = TABS.to_vec();
                let _ = counts;
                let content = match self.tab {
                    0 => self.description(&detail.project, colors),
                    1 => self.versions(&detail, colors, cx),
                    _ => self.gallery(&detail.project, colors),
                };
                let view = cx.entity();
                v_flex()
                    .w_full()
                    .gap_5()
                    .child(self.header(&detail.project, detail.owner.as_deref(), colors, cx))
                    .child(kit::toolbar(
                        Some(
                            kit::tabs("detail-tabs", &labels, self.tab, move |index, _, cx| {
                                view.update(cx, |view, cx| view.select_tab(index, cx));
                            })
                            .into_any_element(),
                        ),
                        None,
                    ))
                    .child(kit::entrance(content, ("detail-body", self.tab)))
                    .into_any_element()
            }
        };

        div()
            .id("live-detail")
            .size_full()
            .overflow_y_scroll()
            .text_color(colors.foreground)
            .child(
                crate::theme::content_column()
                    .mx_auto()
                    .pt(TITLE_BAR_HEIGHT + px(12.))
                    .pb(crate::theme::BOTTOM_SAFE_AREA)
                    .gap_4()
                    .child(
                        // Going back is the navigation's job (design language §6).
                        div()
                            .debug_selector(|| "live-detail-body".into())
                            .child(body),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_naturally() {
        assert_eq!(size_label(37), "37 B");
        assert_eq!(size_label(2048), "2 KB");
        assert_eq!(size_label(1_500_000), "1.4 MB");
        assert_eq!(size_label(3 * 1024 * 1024 * 1024), "3.0 GB");
    }

    #[test]
    fn dates_are_days_or_nothing() {
        assert_eq!(date_label("2024-01-05T10:00:00Z"), "2024-01-05");
        assert_eq!(date_label("soon"), "");
        assert_eq!(date_label(""), "");
    }

    #[test]
    fn version_lists_fold_after_three() {
        let v = |n: &[&str]| n.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(versions_label(&v(&["1.21"])), "1.21");
        assert_eq!(
            versions_label(&v(&["1.21", "1.20", "1.19"])),
            "1.21, 1.20, 1.19"
        );
        assert_eq!(
            versions_label(&v(&["1.21", "1.20", "1.19", "1.18", "1.17"])),
            "1.21, 1.20, 1.19 +2"
        );
        assert_eq!(versions_label(&[]), "");
    }

    use gpui::TestAppContext;
    use lumilio_core::{GalleryImage, ProjectLinks, SideSupport, VersionFile};

    fn detail() -> ProjectDetail {
        ProjectDetail {
            project: Project {
                id: "P".to_owned(),
                slug: "cool".to_owned(),
                title: "Cool".to_owned(),
                description: "summary".to_owned(),
                body: "# Hello".to_owned(),
                kind: ProjectKind::Mod,
                categories: vec!["magic".to_owned()],
                loaders: vec!["fabric".to_owned()],
                downloads: 10,
                followers: 2,
                published: String::new(),
                updated: "2026-01-01T00:00:00Z".to_owned(),
                client_side: SideSupport::Required,
                server_side: SideSupport::Unsupported,
                license: Some("MIT".to_owned()),
                links: ProjectLinks::default(),
                gallery: vec![GalleryImage {
                    url: "https://cdn.example/a.png".to_owned(),
                    title: "A".to_owned(),
                    description: String::new(),
                    featured: true,
                    ordering: 0,
                }],
                icon_url: None,
                game_versions: vec!["1.21".to_owned()],
            },
            versions: vec![Version {
                id: "v1".to_owned(),
                project_id: "P".to_owned(),
                name: "Cool 1".to_owned(),
                number: "1".to_owned(),
                channel: ReleaseChannel::Release,
                game_versions: vec!["1.21".to_owned()],
                loaders: vec!["fabric".to_owned()],
                published: "2026-01-01T00:00:00Z".to_owned(),
                files: vec![VersionFile {
                    url: "https://cdn.example/a.jar".to_owned(),
                    filename: "a.jar".to_owned(),
                    primary: true,
                    size: 2048,
                    sha1: None,
                }],
                dependencies: Vec::new(),
                downloads: 5,
                changelog: String::new(),
            }],
            owner: Some("someone".to_owned()),
        }
    }

    #[gpui::test]
    fn each_tab_shows_its_own_content_and_versions_install_what_was_chosen(
        cx: &mut TestAppContext,
    ) {
        use std::cell::RefCell;
        cx.update(gpui_component::init);
        let seen: Rc<RefCell<Vec<DetailIntent>>> = Rc::default();
        let sink = seen.clone();
        let (view, cx) = cx.add_window_view(|_, _| {
            ProjectDetailView::new(
                "cool",
                "https://modrinth.com/mod/cool",
                Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
            )
        });
        view.update(cx, |view, cx| {
            view.set_target(
                Some(InstallTarget {
                    name: "Pack".to_owned(),
                    game_version: "1.21".to_owned(),
                    loader: Loader::Fabric,
                }),
                cx,
            );
            view.set_state(DetailState::Ready(Box::new(detail())), cx);
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("detail-description").is_some());
        assert!(cx.debug_bounds("detail-versions").is_none());

        view.update(cx, |view, cx| view.select_tab(1, cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("detail-versions").is_some(), "versions tab");
        assert!(cx.debug_bounds("detail-description").is_none());

        // Saving the listed version as a file asks for exactly that version.
        let save = cx
            .debug_bounds("detail-version-save-0")
            .expect("a version can be saved as a file");
        cx.simulate_click(save.center(), gpui::Modifiers::none());
        assert!(matches!(
            seen.borrow().last(),
            Some(DetailIntent::SaveAs { version_id, file_name, .. })
                if version_id == "v1" && file_name == "a.jar"
        ));

        view.update(cx, |view, cx| view.select_tab(2, cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("detail-gallery").is_some(), "gallery tab");
        assert!(cx.debug_bounds("detail-versions").is_none());
    }

    /// Not a check: prints the per-frame cost of a long description.
    /// `cargo test -p lumilio-ui detail_frame_cost -- --ignored --nocapture`
    #[gpui::test]
    #[ignore = "a measurement, not a check"]
    fn detail_frame_cost_of_a_long_description(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let (view, cx) = cx.add_window_view(|_, _| {
            ProjectDetailView::new(
                "cool",
                "https://modrinth.com/mod/cool",
                Rc::new(|_, _, _| {}),
            )
        });
        let mut detail = detail();
        detail.project.body = (0..120)
            .map(|n| {
                format!(
                    "## Section {n}\n\nSome **bold** and _italic_ text with a [link](https://example.com) \
                     and `code`, long enough to wrap across a couple of lines in the window.\n\n\
                     - first item\n- second item\n- third item\n"
                )
            })
            .collect::<String>();
        view.update(cx, |view, cx| {
            view.set_state(DetailState::Ready(Box::new(detail)), cx);
        });
        cx.run_until_parked();
        let frames = 30;
        let start = std::time::Instant::now();
        for _ in 0..frames {
            view.update(cx, |_, cx| cx.notify());
            cx.run_until_parked();
        }
        eprintln!(
            "detail frame: {:?} each over {frames} frames",
            start.elapsed() / frames
        );
    }
}
