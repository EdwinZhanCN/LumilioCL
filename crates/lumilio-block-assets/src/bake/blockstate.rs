//! Blockstate files of every game version to one shape.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::pack::{BakedBlockstate, BakedCase, BakedVariant, Condition, namespaced};

pub(super) fn bake_blockstate(json: &Value) -> Result<BakedBlockstate, String> {
    if let Some(variants) = json.get("variants").and_then(Value::as_object) {
        let mut baked = BTreeMap::new();
        for (key, value) in variants {
            baked.insert(state_key(key), variant_list(value)?);
        }
        return Ok(BakedBlockstate::Variants(baked));
    }
    if let Some(cases) = json.get("multipart").and_then(Value::as_array) {
        let cases = cases
            .iter()
            .map(|case| {
                Ok(BakedCase {
                    when: case.get("when").map(condition).transpose()?,
                    apply: variant_list(case.get("apply").ok_or("a case without `apply`")?)?,
                })
            })
            .collect::<Result<_, String>>()?;
        return Ok(BakedBlockstate::Multipart(cases));
    }
    Err("neither `variants` nor `multipart`".to_owned())
}

/// `lit=false,facing=north` → `facing=north,lit=false`.
fn state_key(key: &str) -> String {
    let mut pairs: Vec<&str> = key.split(',').filter(|pair| !pair.is_empty()).collect();
    pairs.sort_by_key(|pair| pair.split_once('=').map_or(*pair, |(name, _)| name));
    pairs.join(",")
}

fn variant_list(value: &Value) -> Result<Vec<BakedVariant>, String> {
    match value {
        Value::Array(items) => items.iter().map(variant).collect(),
        single => Ok(vec![variant(single)?]),
    }
}

fn variant(value: &Value) -> Result<BakedVariant, String> {
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .ok_or("a variant without `model`")?;
    let int = |key: &str| value.get(key).and_then(Value::as_f64).unwrap_or(0.0) as i32;
    Ok(BakedVariant {
        model: namespaced(model),
        x: int("x"),
        y: int("y"),
        uvlock: value
            .get("uvlock")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        weight: value
            .get("weight")
            .and_then(Value::as_u64)
            .map_or(1, |weight| weight as u32),
    })
}

fn condition(value: &Value) -> Result<Condition, String> {
    let object = value
        .as_object()
        .ok_or("a condition that is not an object")?;
    if object.len() == 1 {
        for (key, combine) in [
            ("OR", Condition::Any as fn(_) -> _),
            ("AND", Condition::All),
        ] {
            if let Some(list) = object.get(key).and_then(Value::as_array) {
                return Ok(combine(
                    list.iter().map(condition).collect::<Result<_, _>>()?,
                ));
            }
        }
    }
    object
        .iter()
        .map(|(name, value)| {
            let value = match value {
                Value::String(text) => text.clone(),
                Value::Number(number) => number.to_string(),
                Value::Bool(flag) => flag.to_string(),
                _ => return Err(format!("condition `{name}` is not a value")),
            };
            Ok((name.clone(), value))
        })
        .collect::<Result<_, _>>()
        .map(Condition::Properties)
}
