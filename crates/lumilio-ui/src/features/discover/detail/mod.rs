//! A project's detail page (Discover): description, versions, gallery, a
//! sidebar of facts, and a page per version. Native, rendered from Modrinth's
//! own data; the public web page is only a link away.
//!
//! The content follows Modrinth App's project pages
//! (`apps/app-frontend/src/pages/project/*` and `ProjectSidebar*`,
//! GPL-3.0-only).

mod gallery;
mod header;
mod translation;
mod versions;

#[cfg(test)]
use std::rc::Rc;

#[cfg(test)]
use lumilio_core::{Loader, Project, ProjectDetail, ProjectKind, ReleaseChannel, Version};
pub(super) use project_detail_view::toggle;
pub use project_detail_view::{
    DetailHandler, DetailIntent, DetailState, InstallTarget, ProjectDetailView, VERSIONS_PER_PAGE,
    VersionFilters, date_label, size_label, versions_label,
};
use project_detail_view::{channel_label, file_of, pill, spec_cell};
pub use translation::SummaryQuery;

#[cfg(test)]
pub(crate) mod tests;

mod project_detail_view;
