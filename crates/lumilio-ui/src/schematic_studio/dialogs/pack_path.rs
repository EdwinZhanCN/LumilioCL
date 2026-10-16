//! 输入路径… under 资源来源's 添加… (plan schematic-studio, T36): a resource
//! pack added by its full path, typed or pasted, for a zip or pack folder
//! the system pickers do not reach easily. Only the path is kept, as with
//! the pickers. The dialog stays open until the answer: a path that is not
//! a pack stays in the field, with the reason under it.

use gpui::prelude::*;
use gpui::{App, Context, Entity, SharedString, Subscription, WeakEntity, Window, div};
use gpui_component::input::{Input, InputEvent, InputState};

use super::{DialogForm, Submission, field};
use crate::schematic_studio::StudioView;
use crate::theme::ShellColors;
use crate::tr;

pub(crate) struct PackPathDialog {
    studio: WeakEntity<StudioView>,
    status: Entity<Submission>,
    path: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl PackPathDialog {
    pub(crate) fn new(
        studio: WeakEntity<StudioView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let path = cx.new(|cx| InputState::new(window, cx).placeholder(tr!("studio-sources-path")));
        let status = cx.new(|_| Submission::default());
        let typed =
            cx.subscribe_in(
                &path,
                window,
                |this, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Change => {
                        this.status.update(cx, |status, cx| status.clear(cx));
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. }
                        if this.ready(cx) && !this.status.read(cx).busy =>
                    {
                        this.commit(window, cx);
                    }
                    _ => {}
                },
            );
        Self {
            studio,
            status,
            path,
            _subscriptions: vec![typed],
        }
    }

    /// The path as typed, without the quotes a pasted path may carry.
    fn typed(&self, cx: &App) -> String {
        self.path
            .read(cx)
            .value()
            .trim()
            .trim_matches('"')
            .trim()
            .to_owned()
    }

    #[cfg(test)]
    pub(crate) fn path_input(&self) -> Entity<InputState> {
        self.path.clone()
    }
}

impl DialogForm for PackPathDialog {
    fn status(&self) -> &Entity<Submission> {
        &self.status
    }

    fn ready(&self, cx: &App) -> bool {
        !self.typed(cx).is_empty()
    }

    fn commit_label(&self) -> SharedString {
        tr!("studio-sources-path-add").into()
    }

    fn commit(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let path = std::path::PathBuf::from(self.typed(cx));
        let status = self.status.clone();
        let _ = self.studio.update(cx, |studio, cx| {
            studio.add_typed(path, status, cx);
        });
    }
}

impl Render for PackPathDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = ShellColors::current(cx);
        let busy = self.status.read(cx).busy;
        field(
            tr!("studio-sources-path-label"),
            div()
                .w_full()
                .debug_selector(|| "studio-sources-path".into())
                .child(Input::new(&self.path).disabled(busy)),
            colors,
        )
    }
}
