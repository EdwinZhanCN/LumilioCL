use super::super::panels::CONTENT_KINDS;
use super::super::{InstanceDetailView, InstanceIntent};
use crate::tr;
use gpui::{App, Context, Window};
use gpui_component::WindowExt as _;
use lumilio_core::{ContentList, ProjectKind};

impl InstanceDetailView {
    pub(super) fn content_index(&self) -> usize {
        self.content_kind.min(2)
    }

    pub(super) fn content_list(&self) -> Option<&ContentList> {
        match &self.data.content[self.content_index()] {
            Some(Ok(list)) => Some(list),
            _ => None,
        }
    }

    pub(super) fn content_query(&self, cx: &App) -> String {
        self.fields
            .as_ref()
            .map(|fields| fields.content_search.read(cx).value().to_string())
            .unwrap_or_default()
    }

    /// Delete asks once, saying what goes (§10).
    pub(super) fn confirm_delete_content(
        &mut self,
        kind: ProjectKind,
        files: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || files.is_empty() {
            return;
        }
        let view = cx.entity().downgrade();
        let title = if files.len() == 1 {
            tr!(
                "game-delete-title",
                name = files[0].trim_end_matches(".disabled")
            )
        } else {
            tr!("instance-content-delete-title-many", count = files.len())
        };
        window.open_alert_dialog(cx, move |alert, _, _| {
            let view = view.clone();
            let files = files.clone();
            alert
                .title(title.clone())
                .description(tr!("instance-content-delete-body"))
                .ok_text(tr!("common-delete"))
                .ok_variant(gpui_component::button::ButtonVariant::Danger)
                .cancel_text(tr!("common-cancel"))
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    let files = files.clone();
                    let _ = view.update(cx, |view, cx| {
                        view.selected.clear();
                        view.send(InstanceIntent::DeleteContent { kind, files }, window, cx);
                    });
                    true
                })
        });
    }

    pub(super) fn set_content_enabled(
        &mut self,
        files: Vec<String>,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let kind = CONTENT_KINDS[self.content_index()];
        self.selected.clear();
        self.send(
            InstanceIntent::SetContent {
                kind,
                files,
                enabled,
            },
            window,
            cx,
        );
    }
}
