//! 导出 PNG… (plan schematic-studio T10): size, background and angle in the
//! §10 dialog. The open viewport's worker draws the picture offscreen with
//! `Scene::render`, it is encoded on the background executor, and the host's
//! save dialog chooses where it goes; nothing is written before that.

use gpui::prelude::*;
use gpui::{App, Context, Entity, WeakEntity, Window, div};
use gpui_component::IndexPath;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::select::{SearchableVec, Select, SelectEvent, SelectState};
use gpui_component::{h_flex, v_flex};
use lumilio_schematic_render::{Angle, Frame, View};

use super::dialogs::{self, DialogForm, Submission, field, note};
use super::{StudioView, tree};
use crate::controls::Segments;
use crate::model_view::{Viewer, angle_labels};
use crate::theme::ShellColors;
use crate::toast::Toast;
use crate::tr;

/// The largest side a PNG may have: the viewport's own readback cap.
pub(crate) const MAX_SIDE: u32 = 4096;

/// A side typed into the dialog, if it is one we can draw.
pub(crate) fn side(text: &str) -> Option<u32> {
    text.trim()
        .parse::<u32>()
        .ok()
        .filter(|side| (1..=MAX_SIDE).contains(side))
}

/// The view an export draws: the current camera or a preset, over the panel
/// colour or nothing.
pub(crate) fn export_view(current: View, angle: Option<Angle>, transparent: bool) -> View {
    let view = angle.map_or(current, |angle| current.at(angle));
    if transparent {
        View {
            background: Some([0., 0., 0., 0.]),
            ..view
        }
    } else {
        view
    }
}

/// A rendered frame (blue first) as PNG bytes, alpha kept.
pub(crate) fn encode_png(frame: Frame) -> Result<Vec<u8>, String> {
    let Frame {
        width,
        height,
        mut bgra,
    } = frame;
    for px in bgra.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    let image = image::RgbaImage::from_raw(width, height, bgra)
        .ok_or_else(|| "frame size does not match its pixels".to_owned())?;
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    Ok(png)
}

type Dropdown = Entity<SelectState<SearchableVec<String>>>;

pub(crate) struct ExportDialog {
    studio: WeakEntity<StudioView>,
    viewer: WeakEntity<Viewer>,
    status: Entity<Submission>,
    file: String,
    width: Entity<InputState>,
    height: Entity<InputState>,
    transparent: bool,
    /// `None` is the current view.
    angle: Option<Angle>,
    angles: Dropdown,
}

impl ExportDialog {
    pub(crate) fn new(
        studio: WeakEntity<StudioView>,
        viewer: WeakEntity<Viewer>,
        file: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let width = cx.new(|cx| InputState::new(window, cx).default_value("1920"));
        let height = cx.new(|cx| InputState::new(window, cx).default_value("1080"));
        let status = cx.new(|_| Submission::default());
        for input in [&width, &height] {
            cx.subscribe(input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.status.update(cx, |status, cx| status.clear(cx));
                    cx.notify();
                }
            })
            .detach();
        }
        let labels: Vec<String> = std::iter::once(tr!("studio-png-angle-current"))
            .chain(angle_labels().iter().copied())
            .map(str::to_owned)
            .collect();
        let angles = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(labels.clone()),
                Some(IndexPath::default()),
                window,
                cx,
            )
        });
        cx.subscribe(
            &angles,
            move |this, _, event: &SelectEvent<SearchableVec<String>>, cx| {
                if let SelectEvent::Confirm(Some(label)) = event
                    && let Some(index) = labels.iter().position(|text| text == label)
                {
                    this.angle = index.checked_sub(1).map(|index| Angle::ALL[index]);
                    cx.notify();
                }
            },
        )
        .detach();
        Self {
            studio,
            viewer,
            status,
            file,
            width,
            height,
            transparent: false,
            angle: None,
            angles,
        }
    }

    fn size(&self, cx: &App) -> Option<(u32, u32)> {
        Some((
            side(&self.width.read(cx).value())?,
            side(&self.height.read(cx).value())?,
        ))
    }

    #[cfg(test)]
    pub(crate) fn set_size(&self, width: &str, height: &str, window: &mut Window, cx: &mut App) {
        self.width
            .update(cx, |input, cx| input.set_value(width, window, cx));
        self.height
            .update(cx, |input, cx| input.set_value(height, window, cx));
    }

    #[cfg(test)]
    pub(crate) fn set_choice(&mut self, angle: Option<Angle>, transparent: bool) {
        self.angle = angle;
        self.transparent = transparent;
    }
}

impl DialogForm for ExportDialog {
    fn status(&self) -> &Entity<Submission> {
        &self.status
    }

    fn ready(&self, cx: &App) -> bool {
        self.size(cx).is_some()
    }

    fn commit_label(&self) -> gpui::SharedString {
        tr!("studio-png-export").into()
    }

    fn commit(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let Some((width, height)) = self.size(cx) else {
            return;
        };
        let answer = self.viewer.upgrade().and_then(|viewer| {
            let viewer = viewer.read(cx);
            if !viewer.is_ready() {
                return None;
            }
            let view = export_view(viewer.view(), self.angle, self.transparent);
            viewer.export(view, width, height)
        });
        let Some(answer) = answer else {
            self.status.update(cx, |status, cx| {
                status.fail(tr!("studio-png-not-ready"), None, cx)
            });
            return;
        };
        self.status.update(cx, |status, cx| status.start(cx));
        let (status, studio) = (self.status.clone(), self.studio.clone());
        let name = format!("{}.png", tree::stem(&self.file));
        cx.spawn(async move |_, cx| {
            let failure = match answer.recv().await {
                // The viewport went (another file was chosen) before drawing.
                Err(_) => Some((tr!("studio-png-closed"), None)),
                Ok(Err(error)) => Some((tr!("studio-png-failed"), Some(error.to_string()))),
                Ok(Ok(frame)) => {
                    let encoded = cx
                        .background_executor()
                        .spawn(async move { encode_png(frame) })
                        .await;
                    match encoded {
                        Ok(png) => {
                            let _ = studio.update(cx, |studio, cx| {
                                studio.close_dialog = true;
                                studio.save_png(name, png, cx);
                                cx.notify();
                            });
                            None
                        }
                        Err(detail) => Some((tr!("studio-png-failed"), Some(detail))),
                    }
                }
            };
            status.update(cx, |status, cx| match failure {
                Some((sentence, detail)) => status.fail(sentence, detail, cx),
                None => status.finish(cx),
            });
        })
        .detach();
    }
}

impl Render for ExportDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = ShellColors::current(cx);
        let busy = self.status.read(cx).busy;
        let invalid = self.size(cx).is_none();
        let this = cx.weak_entity();
        // ia[plugin.litematica]: 选择 PNG 背景 | 导出 PNG 对话框「背景」分段：面板色 / 透明 | 透明时画面以外的像素 alpha 为 0
        let background = Segments::new(
            "studio-png-background",
            &[
                tr!("studio-png-background-panel"),
                tr!("studio-png-background-clear"),
            ],
            usize::from(self.transparent),
            move |index, _, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.transparent = index == 1;
                    cx.notify();
                });
            },
        );
        v_flex()
            .w_full()
            .gap_4()
            .child(
                h_flex()
                    .w_full()
                    .gap_3()
                    .child(
                        div().flex_1().child(field(
                            tr!("studio-png-width"),
                            div()
                                .w_full()
                                .debug_selector(|| "studio-png-width".into())
                                .child(Input::new(&self.width).disabled(busy)),
                            colors,
                        )),
                    )
                    .child(div().flex_1().child(field(
                        tr!("studio-png-height"),
                        Input::new(&self.height).disabled(busy),
                        colors,
                    ))),
            )
            .child(
                div()
                    .debug_selector(|| "studio-png-size-note".into())
                    .child(note(
                        if invalid {
                            tr!("studio-png-size-invalid")
                        } else {
                            tr!("studio-png-size-hint")
                        },
                        colors,
                    )),
            )
            .child(field(tr!("studio-png-background"), background, colors))
            .child(field(
                tr!("studio-png-angle"),
                Select::new(&self.angles).disabled(busy),
                colors,
            ))
    }
}

impl StudioView {
    pub(super) fn open_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(file), Some(viewer)) = (self.selected.clone(), self.viewer.as_ref()) else {
            return;
        };
        let (studio, viewer) = (self.this.clone(), viewer.downgrade());
        let form = cx.new(|cx| ExportDialog::new(studio, viewer, file, window, cx));
        self.export = Some(form.downgrade());
        dialogs::show(form, tr!("studio-png-title"), window, cx);
    }

    /// The encoded picture: the person chooses where it goes, then it is
    /// written off the UI thread.
    pub(super) fn save_png(&mut self, name: String, png: Vec<u8>, cx: &mut Context<Self>) {
        let start = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map_or_else(std::env::temp_dir, std::path::PathBuf::from);
        let chosen = crate::platform::pick_save_path(cx, &start, &name);
        cx.spawn(async move |this, cx| {
            let Some(path) = chosen.await else {
                return;
            };
            let target = path.clone();
            let written = cx
                .background_executor()
                .spawn(async move { std::fs::write(&target, png) })
                .await;
            let toast = match written {
                Ok(()) => {
                    Toast::success(tr!("studio-png-saved", path = path.display().to_string()))
                }
                Err(error) => {
                    Toast::error(tr!("studio-png-save-failed")).technical(error.to_string())
                }
            };
            let _ = this.update(cx, |studio, cx| {
                studio.toasts.push(toast);
                cx.notify();
            });
        })
        .detach();
    }
}
