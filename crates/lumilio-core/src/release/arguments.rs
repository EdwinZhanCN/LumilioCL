use std::collections::BTreeMap;

use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, de};
use serde_json::Value;

use crate::environment::{CompatibilityRule, HostProfile};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArgumentTemplate {
    Literal(String),
    Conditional {
        rules: Vec<CompatibilityRule>,
        values: Vec<String>,
    },
}

impl Serialize for ArgumentTemplate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Literal(value) => serializer.serialize_str(value),
            Self::Conditional { rules, values } => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("rules", rules)?;
                map.serialize_entry("value", values)?;
                map.end()
            }
        }
    }
}

impl ArgumentTemplate {
    pub(crate) fn render(
        &self,
        host: &HostProfile,
        variables: &BTreeMap<String, String>,
    ) -> Vec<String> {
        match self {
            Self::Literal(value) => vec![expand_variables(value, variables)],
            Self::Conditional { rules, values } if host.allows(rules) => values
                .iter()
                .map(|value| expand_variables(value, variables))
                .collect(),
            Self::Conditional { .. } => Vec::new(),
        }
    }
}

impl<'de> Deserialize<'de> for ArgumentTemplate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::String(literal) => Ok(Self::Literal(literal)),
            Value::Object(mut object) => {
                let rules = object
                    .remove("rules")
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(de::Error::custom)?
                    .unwrap_or_default();
                let raw_values = object
                    .remove("value")
                    .or_else(|| object.remove("values"))
                    .ok_or_else(|| de::Error::missing_field("value"))?;
                let values = match raw_values {
                    Value::String(value) => vec![value],
                    Value::Array(values) => values
                        .into_iter()
                        .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                        .collect(),
                    other => {
                        return Err(de::Error::custom(format!(
                            "argument value must be a string or string array, got {other}"
                        )));
                    }
                };
                Ok(Self::Conditional { rules, values })
            }
            other => Err(de::Error::custom(format!(
                "argument must be a string or object, got {other}"
            ))),
        }
    }
}

pub(crate) fn expand_variables(template: &str, variables: &BTreeMap<String, String>) -> String {
    let mut output = String::with_capacity(template.len());
    let mut remaining = template;
    while let Some(start) = remaining.find("${") {
        output.push_str(&remaining[..start]);
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find('}') else {
            output.push_str(&remaining[start..]);
            return output;
        };
        let name = &after_start[..end];
        if let Some(value) = variables.get(name) {
            output.push_str(value);
        } else {
            output.push_str("${");
            output.push_str(name);
            output.push('}');
        }
        remaining = &after_start[end + 1..];
    }
    output.push_str(remaining);
    output
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArgumentGroups {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) game: Option<Vec<ArgumentTemplate>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) jvm: Option<Vec<ArgumentTemplate>>,
}

impl ArgumentGroups {
    pub(crate) fn game(&self) -> Option<&[ArgumentTemplate]> {
        self.game.as_deref()
    }

    pub(crate) fn jvm(&self) -> Option<&[ArgumentTemplate]> {
        self.jvm.as_deref()
    }

    pub(super) fn merge(parent: Option<&Self>, child: Option<&Self>) -> Option<Self> {
        if parent.is_none() && child.is_none() {
            return None;
        }
        Some(Self {
            game: merge_argument_list(
                parent.and_then(|value| value.game.as_deref()),
                child.and_then(|value| value.game.as_deref()),
            ),
            jvm: merge_argument_list(
                parent.and_then(|value| value.jvm.as_deref()),
                child.and_then(|value| value.jvm.as_deref()),
            ),
        })
    }
}

pub(super) fn merge_argument_list(
    parent: Option<&[ArgumentTemplate]>,
    child: Option<&[ArgumentTemplate]>,
) -> Option<Vec<ArgumentTemplate>> {
    if parent.is_none() && child.is_none() {
        return None;
    }
    let mut merged = Vec::new();
    if let Some(parent) = parent {
        merged.extend(parent.iter().cloned());
    }
    if let Some(child) = child {
        merged.extend(child.iter().cloned());
    }
    Some(merged)
}
