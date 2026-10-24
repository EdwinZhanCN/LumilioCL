//! Generates `docs/ia/paths/*.md` from `// ia[page]:` comments in the code
//! (ADR 0019).
//!
//! ```text
//! // ia[library]: 排序 / 按加载器筛选 | L4 两个下拉 | 视图状态，记在偏好设置里
//! ```
//!
//! The fields are action, layer / component, result and an optional note,
//! separated by `|`. A path appears in the generated file if and only if
//! its comment exists, so every generated row means "built".

pub mod plans;

#[cfg(test)]
use std::collections::BTreeMap;
#[cfg(test)]
use std::path::Path;

pub use ia::{
    IaPath, PAGES, Page, generate, parse_line, render, render_index, scan, stale, validate, write,
};

#[cfg(test)]
mod tests;

mod ia;
