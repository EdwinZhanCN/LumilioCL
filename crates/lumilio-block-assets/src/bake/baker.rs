use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use serde_json::Value;

use super::{blockstate, colormap, model};
use crate::pack::{
    Animation, BakedAssets, BakedBlockstate, BakedPack, BakedTexture, Image, PackMeta,
    PaintingVariant, SourceKind, SourceStamp, namespaced,
};
use crate::source::{AssetSource, Category, Entry};
use crate::{BakeError, source};

/// Textures kept whether or not a model names them: folders (ending in `/`)
/// and single files. Block textures, the biome colormaps, the textures block
/// entities and mobs are drawn with, the particles drawn over torches and
/// campfires, the font of sign text and the container background of shown
/// inventories. Other textures are kept only when a model uses them.
pub(super) const KEPT_TEXTURES: [&str; 7] = [
    "block/",
    "colormap/",
    "entity/",
    // Paintings, by variant (plan schematic-studio T35).
    "painting/",
    "particle/",
    "font/ascii",
    "gui/container/generic_54",
];

/// Entity textures the game has moved or renamed, old id → the newest id.
/// Only code names these files, so the baker settles one name for every
/// version. Adding a version's move is a line here.
pub(super) static RENAMES: &str = include_str!("../../data/texture_renames.json");

pub(super) fn renames() -> &'static BTreeMap<String, String> {
    static TABLE: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    TABLE.get_or_init(|| serde_json::from_str(RENAMES).unwrap_or_default())
}

/// The newest id of the texture `id` (namespaced).
pub fn canonical_texture(id: String) -> String {
    renames().get(&id).cloned().unwrap_or(id)
}

/// Bakes `sources`, lowest priority first: a later source's file replaces an
/// earlier source's file of the same name before anything is resolved, so a
/// resource pack that changes a parent model changes every child. The first
/// source must be readable; a later one that is not a resource archive (a
/// mod jar that is no zip, say) is left out with a note. Blocking.
pub fn bake(sources: &[AssetSource]) -> Result<BakedPack, BakeError> {
    bake_layers(None, sources)
}

/// Bakes the overlays `sources` (mod jars, resource packs, lowest priority
/// first) on top of the already baked `base`, which they are laid over by
/// id afterwards. Models whose parents are in the base resolve against the
/// base's [`BakedAssets::parents`] (its templates as written) or, failing
/// that, its flattened models, so a mod block inheriting
/// `minecraft:block/cube_all` keeps its geometry without the client jar
/// (plan schematic-studio, T17). The result holds only what the overlays
/// define. Unreadable overlays are left out with a note. Blocking.
pub fn bake_over(base: &BakedPack, sources: &[AssetSource]) -> Result<BakedPack, BakeError> {
    bake_layers(Some(base), sources)
}

pub(super) fn bake_layers(
    base: Option<&BakedPack>,
    sources: &[AssetSource],
) -> Result<BakedPack, BakeError> {
    let mut meta = PackMeta::default();
    let mut blockstates = BTreeMap::<String, Value>::new();
    let mut models = BTreeMap::<String, Value>::new();
    let mut textures = BTreeMap::<String, Vec<u8>>::new();
    let mut metas = BTreeMap::<String, Vec<u8>>::new();
    let mut paintings = BTreeMap::<String, Value>::new();

    for (index, source) in sources.iter().enumerate() {
        let mut read = match source::read(source) {
            Ok(read) => read,
            Err(error) if base.is_some() || index > 0 => {
                meta.skipped.push(error.to_string());
                meta.sources.push(SourceStamp {
                    kind: match source {
                        AssetSource::Directory(_) => SourceKind::Directory,
                        _ => SourceKind::Archive,
                    },
                    unreadable: true,
                    ..SourceStamp::default()
                });
                continue;
            }
            Err(error) => return Err(error),
        };
        read.stamp.assets = read.entries.len() as u32;
        read.stamp.blocks = read
            .entries
            .iter()
            .filter(|entry| entry.category == Category::Blockstate)
            .count() as u32;
        meta.sources.push(read.stamp);
        if meta.data_version.is_none()
            && let Some(version) = read
                .version_json
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        {
            meta.data_version = version
                .get("world_version")
                .and_then(Value::as_u64)
                .map(|v| v as u32);
            meta.game_version = version.get("id").and_then(Value::as_str).map(str::to_owned);
        }
        let own: BTreeSet<String> = read
            .entries
            .iter()
            .filter(|entry| entry.category == Category::Texture)
            .map(Entry::id)
            .collect();
        for entry in read.entries {
            let id = entry.id();
            match entry.category {
                Category::Blockstate | Category::Model => {
                    let map = if entry.category == Category::Blockstate {
                        &mut blockstates
                    } else {
                        &mut models
                    };
                    match serde_json::from_slice::<Value>(&entry.bytes) {
                        Ok(json) => {
                            map.insert(id, json);
                        }
                        Err(error) => meta.skipped.push(format!("{id}: {error}")),
                    }
                }
                Category::Texture | Category::TextureMeta => {
                    // An old name gives way to the new one in the same source.
                    let canonical = canonical_texture(id.clone());
                    if canonical != id && own.contains(&canonical) {
                        continue;
                    }
                    let map = if entry.category == Category::Texture {
                        &mut textures
                    } else {
                        &mut metas
                    };
                    map.insert(canonical, entry.bytes);
                }
                Category::PaintingVariant => match serde_json::from_slice::<Value>(&entry.bytes) {
                    Ok(json) => {
                        paintings.insert(id, json);
                    }
                    Err(error) => meta.skipped.push(format!("{id}: {error}")),
                },
            }
        }
    }

    let mut assets = BakedAssets::default();
    let mut used_models = BTreeSet::new();
    for (id, json) in &blockstates {
        match blockstate::bake_blockstate(json) {
            Ok(baked) => {
                used_models.extend(variants(&baked).map(|variant| variant.model.clone()));
                assets.blockstates.insert(id.clone(), baked);
            }
            Err(reason) => meta.skipped.push(format!("{id}: {reason}")),
        }
    }
    // Items held by block entities and entities are drawn with their item
    // models, which no blockstate names.
    used_models.extend(
        models
            .keys()
            .filter(|id| {
                id.split_once(':')
                    .is_some_and(|(_, path)| path.starts_with("item/"))
            })
            .cloned(),
    );
    // Every model some model inherits from is kept as written, so a later
    // bake over this pack can resolve against it.
    let named_parents: BTreeSet<String> = models
        .values()
        .filter_map(|json| json.get("parent").and_then(Value::as_str))
        .filter(|parent| !parent.starts_with("builtin/"))
        .map(namespaced)
        .collect();
    for id in named_parents {
        if let Some(json) = models.get(&id) {
            let template = model::template(json, &canonical_texture, &mut meta.skipped, &id);
            assets.parents.insert(id, template);
        }
    }
    if let Some(base) = base {
        // A model an overlay replaces is drawn by the base's blockstates,
        // which the overlay does not repeat. (A replaced template does not
        // reach the base's flattened children; a bake with the client jar
        // does that.)
        used_models.extend(
            models
                .keys()
                .filter(|id| base.assets.models.contains_key(*id))
                .cloned(),
        );
        borrow_parents(&mut models, base, &used_models);
    }
    for id in used_models {
        match model::bake_model(&id, &models, &canonical_texture, &mut meta.skipped) {
            Ok(baked) => {
                assets.models.insert(id, baked);
            }
            Err(reason) => meta.skipped.push(format!("{id}: {reason}")),
        }
    }

    let mut wanted: BTreeSet<&str> = textures
        .keys()
        .filter(|id| {
            let path = id.split_once(':').map_or(id.as_str(), |(_, path)| path);
            KEPT_TEXTURES
                .iter()
                .any(|kept| match kept.strip_suffix('/') {
                    Some(_) => path.starts_with(kept),
                    None => path == *kept,
                })
        })
        .map(String::as_str)
        .collect();
    for model in assets.models.values() {
        wanted.extend(model.textures.values().map(String::as_str));
        wanted.extend(
            model
                .elements
                .iter()
                .flat_map(|element| element.faces.values())
                .filter_map(|face| face.texture.as_deref()),
        );
    }
    for id in wanted {
        let Some(png) = textures.get(id) else {
            continue;
        };
        let image = match decode(png) {
            Ok(image) => image,
            Err(reason) => {
                meta.skipped.push(format!("{id}: {reason}"));
                continue;
            }
        };
        let hash = image.hash();
        assets.images.entry(hash).or_insert(image);
        assets.textures.insert(
            id.to_owned(),
            BakedTexture {
                image: hash,
                animation: metas.get(id).and_then(|bytes| animation(bytes)),
            },
        );
    }

    assets.paintings = painting_variants(&paintings, base.is_none(), &assets);

    for (id, texture) in &assets.textures {
        if let Some(name) = id.strip_prefix("minecraft:colormap/")
            && let Some(sample) = colormap::plains_sample(&assets.images[&texture.image])
        {
            assets.tint_samples.insert(name.to_owned(), sample);
        }
    }

    Ok(BakedPack { meta, assets })
}

/// The paintings the sources define (T35): each `painting_variant` file's
/// size and picture (`asset_id` → `painting/<path>`). Before 1.21 the game
/// data has none and the picture's name is the variant's: a base pack then
/// takes every vanilla picture but the back at 16 pixels a block, measured
/// here, so a resource pack's larger pictures do not change a size.
pub(super) fn painting_variants(
    data: &BTreeMap<String, Value>,
    base: bool,
    assets: &BakedAssets,
) -> BTreeMap<String, PaintingVariant> {
    let mut paintings: BTreeMap<String, PaintingVariant> = data
        .iter()
        .filter_map(|(id, json)| {
            let size = |key: &str| {
                json.get(key)
                    .and_then(Value::as_u64)
                    .and_then(|n| u32::try_from(n).ok())
            };
            let asset = namespaced(json.get("asset_id")?.as_str()?);
            let (namespace, path) = asset.split_once(':')?;
            Some((
                id.clone(),
                PaintingVariant {
                    width: size("width")?,
                    height: size("height")?,
                    texture: format!("{namespace}:painting/{path}"),
                },
            ))
        })
        .collect();
    if base && paintings.is_empty() {
        for (id, texture) in &assets.textures {
            let Some(name) = id.strip_prefix("minecraft:painting/") else {
                continue;
            };
            let image = &assets.images[&texture.image];
            // The back, and the one sheet every painting was cut from
            // before 1.14.
            if name == "back"
                || name.contains('/')
                || !image.width.is_multiple_of(16)
                || !image.height.is_multiple_of(16)
            {
                continue;
            }
            if name.starts_with("paintings_") {
                continue;
            }
            paintings.insert(
                format!("minecraft:{name}"),
                PaintingVariant {
                    width: image.width / 16,
                    height: image.height / 16,
                    texture: id.clone(),
                },
            );
        }
    }
    paintings
}

/// Copies into `models` the base models the chains of `used` reach and the
/// overlays do not define: a template as written when the base kept one,
/// else the base's flattened model.
pub(super) fn borrow_parents(
    models: &mut BTreeMap<String, Value>,
    base: &BakedPack,
    used: &BTreeSet<String>,
) {
    let borrowed = |id: &str| -> Option<Value> {
        let template = match base.assets.parents.get(id) {
            Some(template) => template.clone(),
            None => model::from_flattened(base.assets.models.get(id)?),
        };
        serde_json::to_value(template).ok()
    };
    for start in used {
        let mut next = Some(start.clone());
        for _ in 0..model::MAX_DEPTH {
            let Some(id) = next.take() else { break };
            if !models.contains_key(&id) {
                let Some(json) = borrowed(&id) else { break };
                models.insert(id.clone(), json);
            }
            next = models[&id]
                .get("parent")
                .and_then(Value::as_str)
                .filter(|parent| !parent.starts_with("builtin/"))
                .map(namespaced);
        }
    }
}

pub(super) fn variants(
    blockstate: &BakedBlockstate,
) -> Box<dyn Iterator<Item = &crate::BakedVariant> + '_> {
    match blockstate {
        BakedBlockstate::Variants(map) => Box::new(map.values().flatten()),
        BakedBlockstate::Multipart(cases) => Box::new(cases.iter().flat_map(|case| &case.apply)),
    }
}

impl Entry {
    pub(super) fn id(&self) -> String {
        format!("{}:{}", self.namespace, self.path)
    }
}

pub(super) fn decode(png: &[u8]) -> Result<Image, String> {
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .map_err(|error| error.to_string())?
        .into_rgba8();
    Ok(Image {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    })
}

pub(super) fn animation(mcmeta: &[u8]) -> Option<Animation> {
    let json: Value = serde_json::from_slice(mcmeta).ok()?;
    let section = json.get("animation")?;
    let number = |key: &str| section.get(key).and_then(Value::as_u64).map(|n| n as u32);
    let frames = section.get("frames").and_then(Value::as_array).map(|list| {
        list.iter()
            .filter_map(|frame| match frame {
                Value::Number(index) => Some((index.as_u64()? as u32, None)),
                Value::Object(_) => Some((
                    frame.get("index")?.as_u64()? as u32,
                    frame.get("time").and_then(Value::as_u64).map(|t| t as u32),
                )),
                _ => None,
            })
            .collect()
    });
    Some(Animation {
        frametime: number("frametime").unwrap_or(1),
        interpolate: section
            .get("interpolate")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        width: number("width"),
        height: number("height"),
        frames,
    })
}
