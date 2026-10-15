//! Block models with their parents applied and texture variables followed.
//!
//! The inheritance rules (child textures override the parent's, a child's own
//! elements replace the parent's, `ambientocclusion` is inherited, `builtin/`
//! parents end the chain) follow Schematic-Mesher's
//! `src/resolver/model_resolver.rs` (https://github.com/Schem-at/Schematic-Mesher,
//! AGPL-3.0-only), here applied once while baking instead of on every load.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::pack::{
    BakedElement, BakedFace, BakedModel, Direction, DisplayTransform, ParentModel, Rotation,
    namespaced,
};

/// Deeper parent chains than this are refused (the game's own are under 6).
pub(super) const MAX_DEPTH: usize = 32;

/// Flattens the model `id` from the raw `models`. `rename` canonicalizes
/// texture ids. Problems that leave the model usable are pushed to `notes`.
pub(super) fn bake_model(
    id: &str,
    models: &BTreeMap<String, Value>,
    rename: &dyn Fn(String) -> String,
    notes: &mut Vec<String>,
) -> Result<BakedModel, String> {
    // The chain from the model up to its root.
    let mut chain: Vec<&Map<String, Value>> = Vec::new();
    let mut next = Some(id.to_owned());
    while let Some(current) = next.take() {
        if chain.len() == MAX_DEPTH {
            return Err("its parents loop or nest too deep".to_owned());
        }
        let Some(model) = models.get(&current).and_then(Value::as_object) else {
            if chain.is_empty() {
                return Err("the model is missing".to_owned());
            }
            notes.push(format!("{id}: parent {current} is missing"));
            break;
        };
        chain.push(model);
        next = model
            .get("parent")
            .and_then(Value::as_str)
            .filter(|parent| !parent.starts_with("builtin/"))
            .map(namespaced);
    }

    let mut ambient_occlusion = None;
    let mut variables = BTreeMap::new();
    let mut elements: Option<&Vec<Value>> = None;
    let mut display = BTreeMap::new();
    for model in chain.iter().rev() {
        display.extend(display_of(model.get("display")));
        if let Some(on) = model.get("ambientocclusion").and_then(Value::as_bool) {
            ambient_occlusion = Some(on);
        }
        for (name, value) in model
            .get("textures")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            // 26.x writes `{"sprite": "block/glass", "force_translucent": true}`.
            let text = value
                .as_str()
                .or_else(|| value.get("sprite").and_then(Value::as_str));
            if let Some(text) = text {
                variables.insert(name.clone(), text.to_owned());
            }
        }
        if let Some(own) = model.get("elements").and_then(Value::as_array)
            && !own.is_empty()
        {
            elements = Some(own);
        }
    }

    let resolve = |reference: &str| -> Option<String> {
        let mut current = reference;
        for _ in 0..MAX_DEPTH {
            match current.strip_prefix('#') {
                Some(name) => current = variables.get(name)?,
                None => return Some(rename(namespaced(current))),
            }
        }
        None
    };
    let textures = variables
        .keys()
        .filter_map(|name| Some((name.clone(), resolve(&format!("#{name}"))?)))
        .collect();
    let elements = elements
        .into_iter()
        .flatten()
        .filter_map(|element| match bake_element(element, &resolve) {
            Ok(element) => Some(element),
            Err(reason) => {
                notes.push(format!("{id}: an element was left out: {reason}"));
                None
            }
        })
        .collect();
    Ok(BakedModel {
        ambient_occlusion: ambient_occlusion.unwrap_or(true),
        textures,
        elements,
        display,
    })
}

fn bake_element(
    element: &Value,
    resolve: &dyn Fn(&str) -> Option<String>,
) -> Result<BakedElement, String> {
    let mut faces = BTreeMap::new();
    for (name, face) in element
        .get("faces")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        let direction = Direction::parse(name).ok_or(format!("unknown face `{name}`"))?;
        faces.insert(
            direction,
            BakedFace {
                texture: face
                    .get("texture")
                    .and_then(Value::as_str)
                    .and_then(resolve),
                uv: face.get("uv").map(numbers::<4>).transpose()?,
                cullface: face
                    .get("cullface")
                    .and_then(Value::as_str)
                    .and_then(Direction::parse),
                rotation: face.get("rotation").and_then(Value::as_f64).unwrap_or(0.0) as i32,
                tintindex: face
                    .get("tintindex")
                    .and_then(Value::as_f64)
                    .map(|index| index as i32),
            },
        );
    }
    Ok(BakedElement {
        from: numbers(element.get("from").ok_or("no `from`")?)?,
        to: numbers(element.get("to").ok_or("no `to`")?)?,
        rotation: element.get("rotation").map(rotation).transpose()?,
        shade: element
            .get("shade")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        shade_direction_override: element
            .get("shade_direction_override")
            .and_then(Value::as_str)
            .and_then(Direction::parse),
        light_emission: element
            .get("light_emission")
            .and_then(Value::as_u64)
            .map_or(0, |level| level.min(15) as u8),
        faces,
    })
}

/// The model `json` as a [`ParentModel`]: parsed, in one spelling, with its
/// `#name` references kept for a child to fill.
pub(super) fn template(
    json: &Value,
    rename: &dyn Fn(String) -> String,
    notes: &mut Vec<String>,
    id: &str,
) -> ParentModel {
    let reference = |text: &str| -> Option<String> {
        Some(if text.starts_with('#') {
            text.to_owned()
        } else {
            rename(namespaced(text))
        })
    };
    let textures = json
        .get("textures")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(name, value)| {
            let text = value
                .as_str()
                .or_else(|| value.get("sprite").and_then(Value::as_str))?;
            Some((name.clone(), reference(text)?))
        })
        .collect();
    let elements = json
        .get("elements")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|element| match bake_element(element, &reference) {
            Ok(element) => Some(element),
            Err(reason) => {
                notes.push(format!("{id}: an element was left out: {reason}"));
                None
            }
        })
        .collect();
    ParentModel {
        parent: json.get("parent").and_then(Value::as_str).map(|parent| {
            if parent.starts_with("builtin/") {
                parent.to_owned()
            } else {
                namespaced(parent)
            }
        }),
        ambient_occlusion: json.get("ambientocclusion").and_then(Value::as_bool),
        textures,
        elements,
        display: display_of(json.get("display")),
    }
}

/// A flattened model as a parent: what a model that names one of a pack's
/// kept models (not a template) as its parent inherits. Its faces already
/// name their textures, so a child's texture variables do not reach them.
pub(super) fn from_flattened(model: &BakedModel) -> ParentModel {
    ParentModel {
        parent: None,
        ambient_occlusion: Some(model.ambient_occlusion),
        textures: model.textures.clone(),
        elements: model.elements.clone(),
        display: model.display.clone(),
    }
}

/// A `display` section; a context's missing vectors take their defaults.
fn display_of(display: Option<&Value>) -> BTreeMap<String, DisplayTransform> {
    display
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(context, transform)| {
            let vector = |key: &str, default: f32| {
                transform
                    .get(key)
                    .map(numbers::<3>)
                    .and_then(Result::ok)
                    .unwrap_or([default; 3])
            };
            (
                context.clone(),
                DisplayTransform {
                    rotation: vector("rotation", 0.0),
                    translation: vector("translation", 0.0),
                    scale: vector("scale", 1.0),
                },
            )
        })
        .collect()
}

fn rotation(value: &Value) -> Result<Rotation, String> {
    let number = |key: &str| value.get(key).and_then(Value::as_f64).unwrap_or(0.0) as f32;
    let mut angles = [number("x"), number("y"), number("z")];
    // A template (`ParentModel`) writes the baked form.
    if let Some(baked) = value.get("angles") {
        angles = numbers(baked)?;
    }
    if let Some(axis) = value.get("axis").and_then(Value::as_str) {
        let index = match axis {
            "x" => 0,
            "y" => 1,
            "z" => 2,
            _ => return Err(format!("unknown rotation axis `{axis}`")),
        };
        angles = [0.0; 3];
        angles[index] = number("angle");
    }
    Ok(Rotation {
        origin: value
            .get("origin")
            .map(numbers)
            .transpose()?
            .unwrap_or([8.0; 3]),
        angles,
        rescale: value
            .get("rescale")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn numbers<const N: usize>(value: &Value) -> Result<[f32; N], String> {
    let list = value
        .as_array()
        .filter(|list| list.len() == N)
        .ok_or(format!("expected {N} numbers"))?;
    let mut out = [0.0; N];
    for (slot, item) in out.iter_mut().zip(list) {
        *slot = item.as_f64().ok_or(format!("expected {N} numbers"))? as f32;
    }
    Ok(out)
}
