use super::*;

/// A skin picture's size in the list, and a cape's.
pub(super) const SKIN_THUMB: (f32, f32) = (112., 168.);
pub(super) const CAPE_THUMB: (f32, f32) = (56., 84.);
/// Pictures are drawn at twice their size so they stay sharp on HiDPI.
const THUMB_SCALE: f32 = 2.;

pub(super) fn arms(model: SkinModel) -> Arms {
    match model {
        SkinModel::Wide => Arms::Classic,
        SkinModel::Slim => Arms::Slim,
    }
}

pub(super) fn texture(pixels: &lumilio_core::SkinPixels) -> Option<Arc<Texture>> {
    Texture::new(pixels.width, pixels.height, pixels.rgba.clone()).map(Arc::new)
}

fn picture(frame: lumilio_skin_render::Frame) -> Option<Arc<RenderImage>> {
    image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra)
        .map(|buffer| Arc::new(RenderImage::new([image::Frame::new(buffer)])))
}

impl Wardrobe {
    /// The pictures behind the library and the owned capes. A picture
    /// already drawn for the same skin and arm model is kept; the rest are
    /// drawn off the UI thread.
    pub fn pictures(
        &mut self,
        skins: Vec<(String, AccountLook)>,
        capes: Vec<(String, lumilio_core::SkinPixels)>,
        cx: &mut Context<Self>,
    ) {
        self.update_pictures(skins, Some(capes), cx);
    }

    pub fn update_pictures(
        &mut self,
        skins: Vec<(String, AccountLook)>,
        capes: Option<Vec<(String, lumilio_core::SkinPixels)>>,
        cx: &mut Context<Self>,
    ) {
        self.picture_epoch = self.picture_epoch.wrapping_add(1);
        let epoch = self.picture_epoch;
        self.skins = skins
            .iter()
            .filter_map(|(id, look)| Some((id.clone(), texture(look.skin.as_ref()?)?)))
            .collect();
        let capes_replaced = capes.is_some();
        if let Some(capes) = capes {
            self.capes = capes
                .iter()
                .filter_map(|(id, pixels)| Some((id.clone(), texture(pixels)?)))
                .collect();
        }
        if self.editor.is_some() {
            self.preview_editor(cx);
        }
        let wanted_skins: Vec<((String, SkinModel), Arc<Texture>)> = skins
            .iter()
            .filter_map(|(id, look)| {
                let key = (id.clone(), look.model);
                let skin = self.skins.get(id)?.clone();
                let current = self.skin_pictures.contains_key(&key)
                    && self
                        .skin_sources
                        .get(&key)
                        .is_some_and(|drawn| drawn == &skin);
                (!current).then_some((key, skin))
            })
            .collect();
        let wanted_capes: Vec<(String, Arc<Texture>)> = if capes_replaced {
            self.capes
                .iter()
                .filter_map(|(id, cape)| {
                    let current = self.cape_pictures.contains_key(id)
                        && self.cape_sources.get(id).is_some_and(|drawn| drawn == cape);
                    (!current).then_some((id.clone(), cape.clone()))
                })
                .collect()
        } else {
            Vec::new()
        };
        // Pictures of skins and capes that are gone, or whose texture changed, leave the cache.
        let redraw_skins: std::collections::HashSet<_> =
            wanted_skins.iter().map(|(key, _)| key.clone()).collect();
        let keep_skins: std::collections::HashSet<_> = skins
            .iter()
            .map(|(id, look)| (id.clone(), look.model))
            .filter(|key| !redraw_skins.contains(key))
            .collect();
        let (kept, dropped): (HashMap<_, _>, HashMap<_, _>) =
            std::mem::take(&mut self.skin_pictures)
                .into_iter()
                .partition(|(key, _)| keep_skins.contains(key));
        self.skin_pictures = kept;
        self.old_pictures.extend(dropped.into_values());
        self.skin_sources.retain(|key, _| keep_skins.contains(key));
        if capes_replaced {
            let redraw_capes: std::collections::HashSet<_> =
                wanted_capes.iter().map(|(id, _)| id.clone()).collect();
            let (kept, dropped): (HashMap<_, _>, HashMap<_, _>) =
                std::mem::take(&mut self.cape_pictures)
                    .into_iter()
                    .partition(|(id, _)| self.capes.contains_key(id) && !redraw_capes.contains(id));
            self.cape_pictures = kept;
            self.old_pictures.extend(dropped.into_values());
            self.cape_sources
                .retain(|id, _| self.capes.contains_key(id) && !redraw_capes.contains(id));
        }
        if wanted_skins.is_empty() && wanted_capes.is_empty() {
            cx.notify();
            return;
        }
        self.drawing = Some(cx.spawn(async move |this, cx| {
            let (skins, capes) = cx
                .background_executor()
                .spawn(async move {
                    let size =
                        |(w, h): (f32, f32)| ((w * THUMB_SCALE) as u32, (h * THUMB_SCALE) as u32);
                    let (sw, sh) = size(SKIN_THUMB);
                    let skins: Vec<_> = wanted_skins
                        .into_iter()
                        .map(|((id, model), skin)| {
                            let frame = {
                                let player = Player {
                                    skin: Some(&skin),
                                    arms: arms(model),
                                    cape: None,
                                    back_equipment: Default::default(),
                                    outer_layer: true,
                                };
                                render(&player, Camera::HOME, sw, sh)
                            };
                            ((id, model), skin, frame)
                        })
                        .collect();
                    let (cw, ch) = size(CAPE_THUMB);
                    let back = Camera {
                        yaw: std::f32::consts::PI + 0.35,
                        pitch: 0.1,
                        zoom: 1.0,
                    };
                    let capes: Vec<_> = wanted_capes
                        .into_iter()
                        .map(|(id, cape)| {
                            let frame = {
                                let player = Player {
                                    skin: None,
                                    arms: Arms::Classic,
                                    cape: Some(&cape),
                                    back_equipment: Default::default(),
                                    outer_layer: true,
                                };
                                render(&player, back, cw, ch)
                            };
                            (id, cape, frame)
                        })
                        .collect();
                    (skins, capes)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.picture_epoch != epoch {
                    for (_, _, frame) in skins {
                        if let Some(image) = picture(frame) {
                            this.old_pictures.push(image);
                        }
                    }
                    for (_, _, frame) in capes {
                        if let Some(image) = picture(frame) {
                            this.old_pictures.push(image);
                        }
                    }
                    cx.notify();
                    return;
                }
                this.drawing = None;
                for (key, skin, frame) in skins {
                    if let Some(image) = picture(frame) {
                        if let Some(old) = this.skin_pictures.insert(key.clone(), image) {
                            this.old_pictures.push(old);
                        }
                        this.skin_sources.insert(key, skin);
                    }
                }
                for (id, cape, frame) in capes {
                    if let Some(image) = picture(frame) {
                        if let Some(old) = this.cape_pictures.insert(id.clone(), image) {
                            this.old_pictures.push(old);
                        }
                        this.cape_sources.insert(id, cape);
                    }
                }
                cx.notify();
            });
        }));
    }
}
