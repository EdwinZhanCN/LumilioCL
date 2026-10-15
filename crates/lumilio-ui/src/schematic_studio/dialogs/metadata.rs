//! 编辑元数据 (plan schematic-studio, T15): name, author and description in
//! the §10 dialog. A blank name is refused in the dialog, which stays open
//! with the draft; block data is never touched.

use gpui::prelude::*;
use gpui::{App, Context, Entity, SharedString, WeakEntity, Window, div};
use gpui_component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_component::v_flex;
use lumilio_core::SchematicOperation;
use lumilio_schematic::metadata::{Edit, Info};

use super::{DialogForm, Submission, field};
use crate::schematic_studio::writes::Edited;
use crate::schematic_studio::{StudioView, tree};
use crate::theme::ShellColors;
use crate::tr;

pub(crate) struct MetadataDialog {
    studio: WeakEntity<StudioView>,
    status: Entity<Submission>,
    file: String,
    name: Entity<InputState>,
    author: Entity<InputState>,
    description: Entity<TextareaState>,
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

impl MetadataDialog {
    pub(crate) fn new(
        studio: WeakEntity<StudioView>,
        file: String,
        info: &Info,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).default_value(info.name.clone()));
        let author = cx.new(|cx| InputState::new(window, cx).default_value(info.author.clone()));
        let description = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 6)
                .default_value(info.description.clone())
        });
        let status = cx.new(|_| Submission::default());
        for input in [&name, &author] {
            cx.subscribe(input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.status.update(cx, |status, cx| status.clear(cx));
                }
            })
            .detach();
        }
        Self {
            studio,
            status,
            file,
            name,
            author,
            description,
        }
    }

    pub(crate) fn edit(&self, cx: &App) -> Edit {
        Edit {
            name: self.name.read(cx).value().to_string(),
            author: self.author.read(cx).value().to_string(),
            description: self.description.read(cx).value().to_string(),
        }
    }

    #[cfg(test)]
    pub(crate) fn name_input(&self) -> Entity<InputState> {
        self.name.clone()
    }
}

impl DialogForm for MetadataDialog {
    fn status(&self) -> &Entity<Submission> {
        &self.status
    }

    fn ready(&self, _: &App) -> bool {
        true
    }

    fn commit_label(&self) -> SharedString {
        tr!("common-save").into()
    }

    fn commit(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let edit = self.edit(cx);
        if edit.name.trim().is_empty() {
            self.status.update(cx, |status, cx| {
                status.fail(tr!("studio-meta-name-empty"), None, cx)
            });
            return;
        }
        let name = edit.name.trim().to_owned();
        let file = tree::stem(&self.file).to_owned();
        let status = self.status.clone();
        let work = move |bytes: &[u8]| {
            let mut schematic =
                lumilio_schematic::load_litematic(bytes).map_err(|error| error.to_string())?;
            lumilio_schematic::metadata::apply(&mut schematic, &edit, now_millis())
                .map_err(|error| error.to_string())?;
            let bytes = lumilio_schematic::to_litematic_bytes(&schematic)
                .map_err(|error| error.to_string())?;
            Ok(Edited { bytes, count: 1 })
        };
        let describe = Box::new(move |_: u64| {
            (
                SchematicOperation {
                    kind: "metadata".into(),
                    summary: tr!("studio-meta-summary", name = name.as_str()),
                },
                tr!("studio-meta-saved", file = file.as_str()),
            )
        });
        let _ = self.studio.update(cx, |studio, cx| {
            studio.submit(status, work, describe, cx);
        });
    }
}

impl Render for MetadataDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = ShellColors::current(cx);
        let busy = self.status.read(cx).busy;
        v_flex()
            .w_full()
            .gap_4()
            .child(field(
                tr!("studio-meta-name"),
                div()
                    .w_full()
                    .debug_selector(|| "studio-meta-name-field".into())
                    .child(Input::new(&self.name).disabled(busy)),
                colors,
            ))
            .child(field(
                tr!("studio-meta-author"),
                Input::new(&self.author).disabled(busy),
                colors,
            ))
            .child(field(
                tr!("studio-meta-description"),
                Textarea::new(&self.description).disabled(busy),
                colors,
            ))
    }
}
