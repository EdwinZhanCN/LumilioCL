use gpui::Context;
use lumilio_core::ProjectKind;

use super::LauncherShell;
use crate::features::discover::OpenedProject;
use crate::features::discover::detail::{DetailState, ProjectDetailView};
use crate::route::Route;

/// A project detail on screen or in history.
pub(super) type DetailSlot = OpenedProject;

impl LauncherShell {
    /// Opens a project's detail in place of the Discover list. It starts as
    /// "loading" and fills in when the project arrives.
    pub fn open_project(
        &mut self,
        kind: ProjectKind,
        source: &str,
        slug: &str,
        cx: &mut Context<Self>,
    ) {
        let (source, slug) = (source.to_owned(), slug.to_owned());
        let opened = self.discover.update(cx, |discover, cx| {
            discover.open_project(kind, source, slug, cx)
        });
        self.show_project(opened, cx);
    }

    /// Shows a detail Discover opened, recording where the window was.
    pub(super) fn show_project(&mut self, opened: OpenedProject, cx: &mut Context<Self>) {
        self.visit(cx);
        self.live_instance = None;
        self.detail = Some(opened);
        self.route = Route::Discover;
        cx.notify();
    }

    /// Shows a gallery image of the open project large.
    pub fn open_gallery_image(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(slot) = &self.detail {
            slot.view.update(cx, |view, cx| view.open_viewer(index, cx));
        }
    }

    /// Fills the open detail in, if it is still the one asked for.
    pub fn set_detail_state(&mut self, slug: &str, state: DetailState, cx: &mut Context<Self>) {
        if let Some(slot) = self.detail.as_ref().filter(|slot| slot.slug == slug) {
            slot.view.update(cx, |view, cx| view.set_state(state, cx));
        }
    }

    pub fn select_detail_tab(&mut self, tab: usize, cx: &mut Context<Self>) {
        if let Some(slot) = &self.detail {
            slot.view.update(cx, |view, cx| view.select_tab(tab, cx));
        }
    }

    /// Leaves the open project: back where it came from, or the list.
    pub fn close_detail(&mut self, cx: &mut Context<Self>) {
        if self.detail.is_some() && !self.go_back(None, cx) {
            self.detail = None;
            cx.notify();
        }
    }

    /// The project open on Discover, if any.
    pub fn detail_project(&self) -> Option<(ProjectKind, &str)> {
        self.detail
            .as_ref()
            .map(|slot| (slot.kind, slot.slug.as_str()))
    }

    /// The content source of the project open on Discover, if any.
    pub fn detail_source(&self) -> Option<&str> {
        self.detail.as_ref().map(|slot| slot.source.as_str())
    }

    /// The page of the project open on Discover; compare it to tell whether
    /// the page changed since.
    pub fn detail_view(&self) -> Option<gpui::Entity<ProjectDetailView>> {
        self.detail.as_ref().map(|slot| slot.view.clone())
    }
}

/// How the navigation names a project detail (§6).
pub(super) fn detail_title(kind: ProjectKind) -> &'static str {
    match kind {
        ProjectKind::Modpack => crate::tr!("detail-title-modpack"),
        ProjectKind::Mod => crate::tr!("detail-title-mod"),
        ProjectKind::ResourcePack => crate::tr!("detail-title-resource-pack"),
        ProjectKind::Shader => crate::tr!("detail-title-shader"),
    }
}
