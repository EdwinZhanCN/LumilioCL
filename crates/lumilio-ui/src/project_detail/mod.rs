//! A project's detail page (Discover): description, versions, gallery, a
//! sidebar of facts, and a page per version. Native, rendered from Modrinth's
//! own data; the public web page is only a link away.
//!
//! The content follows Modrinth App's project pages
//! (`apps/app-frontend/src/pages/project/*` and `ProjectSidebar*`,
//! GPL-3.0-only; ADR 0022).

mod gallery;
mod header;
mod versions;

use std::rc::Rc;

use gpui::{App, Context, Div, IntoElement, Render, SharedString, Window, div, prelude::*, px};
use gpui_component::{TITLE_BAR_HEIGHT, v_flex};
use lumilio_core::{
    GameVersionTag, Loader, Project, ProjectDetail, ProjectKind, ReleaseChannel, Version,
    version_fits,
};

use crate::kit;
use crate::live::parse_rfc3339;
use crate::theme::ShellColors;
use crate::tr;

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
    /// Replace the installed file by this version (newer, or another one).
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
    /// Discover was opened from this game's Content tab: the versions list
    /// starts filtered to it, as in the app.
    pub from_game: bool,
}

#[derive(Clone, Debug)]
pub enum DetailState {
    Loading,
    Failed(String),
    Ready(Box<ProjectDetail>),
}

/// What the versions list is narrowed to.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VersionFilters {
    pub channels: Vec<ReleaseChannel>,
    pub game_versions: Vec<String>,
    pub platforms: Vec<String>,
    /// Zero-based.
    pub page: usize,
}

impl VersionFilters {
    pub fn active(&self) -> bool {
        !(self.channels.is_empty() && self.game_versions.is_empty() && self.platforms.is_empty())
    }

    /// Whether a version stays in the list: it passes each filter that is on
    /// (any one of the chosen channels, game versions, platforms).
    pub fn admits(&self, version: &Version) -> bool {
        (self.channels.is_empty() || self.channels.contains(&version.channel))
            && (self.game_versions.is_empty()
                || version
                    .game_versions
                    .iter()
                    .any(|have| self.game_versions.contains(have)))
            && (self.platforms.is_empty()
                || version
                    .loaders
                    .iter()
                    .any(|have| self.platforms.contains(have)))
    }
}

pub(super) fn toggle<T: PartialEq>(list: &mut Vec<T>, item: T) {
    match list.iter().position(|have| *have == item) {
        Some(at) => {
            list.remove(at);
        }
        None => list.push(item),
    }
}

pub struct ProjectDetailView {
    title: SharedString,
    url: SharedString,
    state: DetailState,
    tab: usize,
    target: Option<InstallTarget>,
    /// What the target game already has, by project.
    installed: std::collections::BTreeMap<String, lumilio_core::InstalledProject>,
    /// This project is being installed right now.
    installing: bool,
    /// Messages waiting for the next render to float up (§11).
    toasts: Vec<crate::toast::Toast>,
    handler: DetailHandler,
    /// Modrinth's game version list, for showing versions as ranges.
    game_tags: Vec<GameVersionTag>,
    filters: VersionFilters,
    /// The filters were started from the game once; the person's changes stay.
    prefiltered: bool,
    /// The gallery image shown large.
    viewer: Option<usize>,
    /// The game version picker of the versions list, made on first use.
    version_picker: Option<versions::Picker>,
}

pub const VERSIONS_PER_PAGE: usize = 20;

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
            installing: false,
            toasts: Vec::new(),
            handler,
            game_tags: Vec::new(),
            filters: VersionFilters::default(),
            prefiltered: false,
            viewer: None,
            version_picker: None,
        }
    }

    pub fn set_state(&mut self, state: DetailState, cx: &mut Context<Self>) {
        if let DetailState::Ready(detail) = &state {
            self.title = detail.project.title.clone().into();
        }
        self.state = state;
        self.prefilter();
        cx.notify();
    }

    /// Starts the versions list from the game Discover was opened for: its
    /// loader (for mods) and its game version.
    fn prefilter(&mut self) {
        let (DetailState::Ready(detail), Some(target)) = (&self.state, &self.target) else {
            return;
        };
        if self.prefiltered || !target.from_game {
            return;
        }
        self.prefiltered = true;
        self.filters.game_versions = vec![target.game_version.clone()];
        if detail.project.kind == ProjectKind::Mod
            && let Some(tag) = loader_name(target.loader)
        {
            self.filters.platforms = vec![tag.to_owned()];
        }
    }

    pub fn set_target(&mut self, target: Option<InstallTarget>, cx: &mut Context<Self>) {
        self.target = target;
        self.prefilter();
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

    pub fn set_installing(&mut self, installing: bool, cx: &mut Context<Self>) {
        if self.installing != installing {
            self.installing = installing;
            cx.notify();
        }
    }

    pub fn set_game_tags(&mut self, tags: Vec<GameVersionTag>, cx: &mut Context<Self>) {
        if self.game_tags != tags {
            self.game_tags = tags;
            cx.notify();
        }
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

    pub fn filters(&self) -> &VersionFilters {
        &self.filters
    }

    /// The tabs this project has: the gallery only when there is one.
    pub fn tabs(&self) -> Vec<&'static str> {
        let mut tabs = vec![tr!("project-tab-about"), tr!("project-tab-versions")];
        if matches!(&self.state, DetailState::Ready(detail) if !detail.project.gallery.is_empty()) {
            tabs.push(tr!("project-tab-gallery"));
        }
        tabs
    }

    pub fn select_tab(&mut self, tab: usize, cx: &mut Context<Self>) {
        self.tab = tab.min(self.tabs().len() - 1);
        cx.notify();
    }

    /// Shows gallery image `index` large.
    pub fn open_viewer(&mut self, index: usize, cx: &mut Context<Self>) {
        self.viewer = Some(index);
        cx.notify();
    }

    pub fn filter_versions(
        &mut self,
        change: impl FnOnce(&mut VersionFilters),
        cx: &mut Context<Self>,
    ) {
        change(&mut self.filters);
        self.filters.page = 0;
        cx.notify();
    }
}

/// The name Modrinth uses for a loader in a version's list.
pub(super) fn loader_name(loader: Loader) -> Option<&'static str> {
    match loader {
        Loader::Vanilla => None,
        Loader::Fabric => Some("fabric"),
        Loader::Forge => Some("forge"),
        Loader::NeoForge => Some("neoforge"),
        Loader::Quilt => Some("quilt"),
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

fn channel_label(channel: ReleaseChannel) -> &'static str {
    match channel {
        ReleaseChannel::Release => tr!("project-channel-release"),
        ReleaseChannel::Beta => tr!("project-channel-beta"),
        ReleaseChannel::Alpha => tr!("project-channel-alpha"),
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
                .font_family(crate::theme::mono_font())
                .text_size(crate::theme::font_px(10.))
                .text_color(colors.muted)
                .child(legend),
        )
        .child(
            div()
                .font_family(crate::theme::mono_font())
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
    pub(super) fn fits_target(&self, project: &Project, version: &Version) -> bool {
        match (&self.target, project.kind) {
            (_, ProjectKind::Modpack) | (None, _) => true,
            (Some(target), kind) => {
                version_fits(version, kind, &target.game_version, target.loader)
            }
        }
    }

    /// The version of this project the target game has, if it has one.
    pub(super) fn installed_version(&self, project: &Project) -> Option<&str> {
        self.target.as_ref()?;
        self.installed
            .get(&project.id)
            .map(|have| have.version_id.as_str())
    }
}

impl Render for ProjectDetailView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::toast::flush(&mut self.toasts, window, cx);
        let colors = ShellColors::current(cx);
        let body = match self.state.clone() {
            DetailState::Loading => {
                kit::empty(tr!("library-loading"), "", colors).into_any_element()
            }
            DetailState::Failed(message) => v_flex()
                .items_center()
                .child(kit::empty(
                    tr!("project-offline"),
                    tr!("project-offline-help"),
                    colors,
                ))
                .child(kit::technical("detail-technical", message))
                .into_any_element(),
            DetailState::Ready(detail) => self.page(&detail, colors, window, cx),
        };

        div().size_full().relative().child(
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
                ),
        )
    }
}

impl ProjectDetailView {
    /// The loaded page: header, tabs, and beside the tab's content the
    /// sidebar of facts.
    fn page(
        &mut self,
        detail: &ProjectDetail,
        colors: ShellColors,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let labels = self.tabs();
        let tab = self.tab.min(labels.len() - 1);
        let main = match tab {
            0 => self.description(&detail.project, colors),
            1 => self.versions(detail, colors, window, cx),
            _ => self.gallery(&detail.project, colors, cx),
        };
        let view = cx.entity();
        v_flex()
            .w_full()
            .gap_5()
            .child(self.header(&detail.project, detail.owner.as_deref(), colors, cx))
            .child(kit::toolbar(
                Some(
                    kit::tabs("detail-tabs", &labels, tab, move |index, _, cx| {
                        view.update(cx, |view, cx| view.select_tab(index, cx));
                    })
                    .into_any_element(),
                ),
                None,
            ))
            .child(kit::entrance(main, ("detail-body", tab)))
            .into_any_element()
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
            .child(gpui_component::text::TextView::markdown("detail-body", text).selectable(true))
            .into_any_element()
    }
}

#[cfg(test)]
pub(crate) mod tests;
