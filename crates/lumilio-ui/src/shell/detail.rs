use super::LauncherShell;
use crate::live::{LiveIntent, LiveModel};
use crate::project_detail::{
    DetailHandler, DetailIntent, DetailState, InstallTarget, ProjectDetailView,
};
use crate::route::Route;
use gpui::prelude::*;
use gpui::{Context, Entity};
use lumilio_core::ProjectKind;
use std::rc::Rc;

#[derive(Clone)]
pub(super) struct DetailSlot {
    pub(super) view: Entity<ProjectDetailView>,
    pub(super) kind: ProjectKind,
    pub(super) slug: String,
}

impl LauncherShell {
    /// Chooses the instance play and installs target, everywhere at once.
    pub fn set_install_target(&mut self, id: String, cx: &mut Context<Self>) {
        let target = self
            .live
            .as_ref()
            .and_then(|model| install_target_of(model, &id));
        if let (Some(slot), Some(target)) = (&self.detail, target) {
            slot.view
                .update(cx, |view, cx| view.set_target(Some(target), cx));
        }
        self.update_live(
            |model| {
                model.install_target = Some(id);
            },
            cx,
        );
    }

    /// Opens a project's detail in place of the Discover list. It starts as
    /// "loading"; [`Self::set_detail_state`] fills it in.
    pub fn open_detail(&mut self, kind: ProjectKind, slug: &str, cx: &mut Context<Self>) {
        let Some(live) = self.live_handler.clone() else {
            return;
        };
        let project = slug.to_owned();
        let handler: DetailHandler = Rc::new(move |intent, window, cx| match intent {
            DetailIntent::SaveAs {
                version_id,
                file_name,
                title,
            } => live(
                LiveIntent::SaveVersionAs {
                    project: project.clone(),
                    version_id,
                    file_name,
                    title,
                },
                window,
                cx,
            ),
            DetailIntent::Update {
                file_name,
                version_id,
                title,
            } => live(
                LiveIntent::UpdateInstalled {
                    kind,
                    project: project.clone(),
                    title,
                    file_name,
                    version_id,
                },
                window,
                cx,
            ),
            DetailIntent::Install { version_id, title } => live(
                LiveIntent::Install {
                    kind,
                    slug: project.clone(),
                    title,
                    version: version_id,
                },
                window,
                cx,
            ),
        });
        let url = lumilio_core::project_page_url(kind, slug);
        let target = self.live.as_ref().and_then(|model| {
            let id = model.install_target.clone()?;
            install_target_of(model, &id)
        });
        let tags = self
            .live
            .as_ref()
            .map(|model| model.filters.game_tags().to_vec())
            .unwrap_or_default();
        let view = cx.new(|_| {
            let mut view = ProjectDetailView::new(slug.to_owned(), url, handler);
            view.set_target_quiet(target);
            view
        });
        view.update(cx, |view, cx| view.set_game_tags(tags, cx));
        self.visit();
        self.live_instance = None;
        self.detail = Some(DetailSlot {
            view,
            kind,
            slug: slug.to_owned(),
        });
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
}

/// How the navigation names a project detail (§6).
pub(super) const fn detail_title(kind: ProjectKind) -> &'static str {
    match kind {
        ProjectKind::Modpack => "整合包详情",
        ProjectKind::Mod => "Mod 详情",
        ProjectKind::ResourcePack => "资源包详情",
        ProjectKind::Shader => "光影详情",
    }
}

/// The game installs go into, as the detail page needs to know it.
fn install_target_of(model: &LiveModel, id: &str) -> Option<InstallTarget> {
    let card = model.library.iter().find(|card| card.id == id)?;
    Some(InstallTarget {
        name: card.name.clone(),
        game_version: card.game_version.clone(),
        loader: card.loader,
        from_game: model.browsing_for.as_deref() == Some(id),
    })
}
