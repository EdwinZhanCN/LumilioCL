use super::LauncherShell;
use crate::live::LiveIntent;
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
        let target = self.live.as_ref().and_then(|model| {
            let card = model.library.iter().find(|card| card.id == id)?;
            Some(InstallTarget {
                name: card.name.clone(),
                game_version: card.game_version.clone(),
                loader: card.loader,
            })
        });
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
            let card = model
                .library
                .iter()
                .find(|card| Some(&card.id) == model.install_target.as_ref())?;
            Some(InstallTarget {
                name: card.name.clone(),
                game_version: card.game_version.clone(),
                loader: card.loader,
            })
        });
        let view = cx.new(|_| {
            let mut view = ProjectDetailView::new(slug.to_owned(), url, handler);
            view.set_target_quiet(target);
            view
        });
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
