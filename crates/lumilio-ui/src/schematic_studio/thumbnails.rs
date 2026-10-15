//! Schematic thumbnails (plan schematic-studio, T28; D29): one picture per
//! file for the catalogue's cards and the switcher's list, drawn from the
//! same block assets as the viewport, from the isometric angle on a clear
//! background. The studio asks for one at a time, keyed by the file's
//! content fingerprint; the application keeps drawn ones, so a file is drawn
//! once until its blocks change.

use std::sync::Arc;

use gpui::{Context, RenderImage};
use lumilio_schematic_render::{Angle, Scene, View};

use super::{Command, StudioView};

/// A thumbnail's physical pixels: 4:3, sharp at the cards' size.
pub const THUMBNAIL_SIZE: (u32, u32) = (320, 240);

/// Draws `preview` as a thumbnail PNG. Meshes the whole schematic: call it
/// off the UI thread.
pub fn render_thumbnail(preview: lumilio_core::ModelPreview) -> Result<Vec<u8>, String> {
    let pack = preview
        .pack
        .ok_or_else(|| "no block assets to draw with".to_owned())?;
    let mut scene = Scene::load(&preview.schematic, &pack, preview.entities)
        .map_err(|error| error.to_string())?;
    let view = View {
        background: Some([0., 0., 0., 0.]),
        ..View::new()
    }
    .at(Angle::Isometric);
    let frame = scene
        .render(&view, THUMBNAIL_SIZE.0, THUMBNAIL_SIZE.1)
        .map_err(|error| error.to_string())?;
    super::export::encode_png(frame)
}

impl StudioView {
    /// Asks for the next thumbnail no file has yet, one at a time.
    pub(super) fn ask_thumbnail(&mut self) {
        if self.thumb_asking.is_some() {
            return;
        }
        while let Some(file) = self.thumb_queue.pop_front() {
            let Some(print) = self.tree.fingerprint(&file) else {
                continue;
            };
            if self.thumbnails.contains_key(&file) || self.thumb_failed.contains(&file) {
                continue;
            }
            self.send(Command::Thumbnail {
                file: file.clone(),
                key: print.to_string(),
            });
            self.thumb_asking = Some(file);
            return;
        }
    }

    /// The sources changed (T21): every thumbnail shown is asked for again,
    /// as the application keeps them per look of the sources switched on.
    pub(super) fn redraw_thumbnails(&mut self) {
        let files: Vec<String> = self.thumbnails.keys().cloned().collect();
        self.old_icons
            .extend(self.thumbnails.drain().map(|(_, thumbnail)| thumbnail));
        self.thumb_failed.clear();
        self.thumb_redo.clone_from(&self.thumb_asking);
        for file in files {
            self.want_thumbnail(&file, false);
        }
    }

    /// The file's blocks are known (or changed): its thumbnail is due.
    pub(super) fn want_thumbnail(&mut self, file: &str, changed: bool) {
        if changed {
            if let Some(old) = self.thumbnails.remove(file) {
                self.old_icons.push(old);
            }
            self.thumb_failed.remove(file);
        }
        if !self.thumbnails.contains_key(file) && !self.thumb_queue.iter().any(|f| f == file) {
            self.thumb_queue.push_back(file.to_owned());
        }
        self.ask_thumbnail();
    }

    pub(super) fn thumbnail_arrived(
        &mut self,
        file: String,
        result: Result<Vec<u8>, String>,
        cx: &mut Context<Self>,
    ) {
        if self.thumb_asking.as_deref() == Some(file.as_str()) {
            self.thumb_asking = None;
        }
        if self.thumb_redo.as_deref() == Some(file.as_str()) {
            self.thumb_redo = None;
            self.want_thumbnail(&file, true);
            return;
        }
        match result {
            Ok(png) => {
                let decode = cx.background_executor().spawn(async move {
                    image::load_from_memory(&png).map(|image| {
                        let mut rgba = image.to_rgba8();
                        // GPUI wants blue first.
                        for px in rgba.as_chunks_mut::<4>().0 {
                            px.swap(0, 2);
                        }
                        Arc::new(RenderImage::new([image::Frame::new(rgba)]))
                    })
                });
                cx.spawn(async move |this, cx| {
                    let decoded = decode.await;
                    let _ = this.update(cx, |this, cx| {
                        match decoded {
                            Ok(picture) => {
                                if let Some(old) = this.thumbnails.insert(file, picture) {
                                    this.old_icons.push(old);
                                }
                            }
                            Err(_) => {
                                this.thumb_failed.insert(file);
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            // No assets, or a file that cannot be drawn: its card keeps the
            // quiet square; nothing to say.
            Err(_) => {
                self.thumb_failed.insert(file);
            }
        }
        self.ask_thumbnail();
    }
}
