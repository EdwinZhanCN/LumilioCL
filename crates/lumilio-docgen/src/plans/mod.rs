//! Versioned plan contracts and deterministic, explicitly public projections.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plan {
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    pub summary: String,
    pub status: Status,
    pub visibility: Visibility,
    pub scope: Scope,
    pub tasks: Vec<Task>,
    pub validation: Vec<Validation>,
    #[serde(default)]
    pub decisions: Vec<Decision>,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    pub outcome: Option<String>,
    #[serde(default)]
    pub lessons_learned: Vec<String>,
    pub roadmap: Option<Roadmap>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Proposed,
    InProgress,
    Blocked,
    Completed,
    Cancelled,
}
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Internal,
    Public,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub included: Vec<String>,
    pub excluded: Vec<String>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub status: Status,
    pub acceptance: String,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Validation {
    pub criterion: String,
    pub result: Option<String>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub id: String,
    pub decision: String,
    pub rationale: String,
    pub consequences: String,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Roadmap {
    pub enabled: bool,
    pub id: String,
    pub name: String,
    pub lede: String,
    pub target_version: Option<String>,
    pub order: u32,
}

fn nonempty(value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err("empty required text".into())
    } else {
        Ok(())
    }
}
fn identifier(value: &str) -> Result<(), String> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        Err(format!("invalid identifier: {value}"))
    } else {
        Ok(())
    }
}

pub fn validate(plans: &[Plan]) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    let mut roadmap_ids = BTreeSet::new();
    for plan in plans {
        identifier(&plan.id)?;
        if plan.schema_version != 1 || !ids.insert(&plan.id) {
            return Err(format!("unsupported schema or duplicate plan: {}", plan.id));
        }
        nonempty(&plan.title)?;
        nonempty(&plan.summary)?;
        if plan.tasks.is_empty() || plan.validation.is_empty() || plan.scope.included.is_empty() {
            return Err(format!("missing tasks, scope or acceptance: {}", plan.id));
        }
        let mut tasks = BTreeSet::new();
        for task in &plan.tasks {
            identifier(&task.id)?;
            if !tasks.insert(&task.id) {
                return Err(format!("duplicate task: {}", task.id));
            }
            nonempty(&task.title)?;
            nonempty(&task.acceptance)?;
        }
        for text in plan
            .scope
            .included
            .iter()
            .chain(&plan.scope.excluded)
            .chain(&plan.references)
            .chain(&plan.lessons_learned)
        {
            nonempty(text)?;
        }
        for entry in &plan.validation {
            nonempty(&entry.criterion)?;
            if let Some(result) = &entry.result {
                nonempty(result)?;
            }
        }
        let mut decisions = BTreeSet::new();
        for entry in &plan.decisions {
            identifier(&entry.id)?;
            if !decisions.insert(&entry.id) {
                return Err("duplicate decision id".into());
            }
            for text in [&entry.decision, &entry.rationale, &entry.consequences] {
                nonempty(text)?;
            }
        }
        if let Some(outcome) = &plan.outcome {
            nonempty(outcome)?;
        }
        if plan.status == Status::Completed
            && (plan.outcome.is_none()
                || plan
                    .tasks
                    .iter()
                    .any(|t| t.status != Status::Completed && t.status != Status::Cancelled)
                || plan.validation.iter().any(|v| v.result.is_none()))
        {
            return Err(format!(
                "completed plan lacks completed work/validation/outcome: {}",
                plan.id
            ));
        }
        if let Some(roadmap) = &plan.roadmap {
            identifier(&roadmap.id)?;
            nonempty(&roadmap.name)?;
            nonempty(&roadmap.lede)?;
            if !roadmap_ids.insert(&roadmap.id) {
                return Err("duplicate roadmap id".into());
            }
            if let Some(version) = &roadmap.target_version {
                nonempty(version)?;
            }
        }
    }
    for plan in plans {
        let mut dependencies = BTreeSet::new();
        for dependency in &plan.dependencies {
            if dependency == &plan.id
                || !ids.contains(dependency)
                || !dependencies.insert(dependency)
            {
                return Err(format!("invalid dependency: {dependency}"));
            }
        }
    }
    Ok(())
}

pub fn load(root: &Path) -> Result<Vec<Plan>, String> {
    let mut paths = fs::read_dir(root.join(".agents/plans"))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect::<Vec<_>>();
    paths.sort();
    let mut plans = Vec::new();
    for path in paths {
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        plans.push(serde_json::from_str(&content).map_err(|e| format!("{}: {e}", path.display()))?);
    }
    validate(&plans)?;
    plans.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(plans)
}

const HEADER: &str =
    "<!-- Generated by lumilio-docgen plans generate; edit .agents/plans/*.json. -->\n";

// Struct field order is stable even when another workspace crate enables
// serde_json/preserve_order. Never serialize a Value map for generated output.
#[derive(Serialize)]
struct PublicRoadmap<'a> {
    _generated: &'a str,
    items: Vec<PublicRoadmapItem<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicRoadmapItem<'a> {
    id: &'a str,
    plan_id: &'a str,
    name: &'a str,
    lede: &'a str,
    version: Option<&'a str>,
    status: &'a Status,
}

pub fn generate(plans: &[Plan]) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    for plan in plans {
        let mut body = format!(
            "{HEADER}\n# {}\n\nStatus: {:?}\n\n{}\n",
            plan.title, plan.status, plan.summary
        );
        body.push_str("\n## Scope\n\n");
        for item in &plan.scope.included {
            body.push_str(&format!("- In: {item}\n"));
        }
        for item in &plan.scope.excluded {
            body.push_str(&format!("- Out: {item}\n"));
        }
        body.push_str("\n## Tasks\n\n");
        for task in &plan.tasks {
            body.push_str(&format!(
                "- [{}] {} — {} ({:?})\n  Acceptance: {}\n",
                if task.status == Status::Completed {
                    "x"
                } else {
                    " "
                },
                task.id,
                task.title,
                task.status,
                task.acceptance
            ));
        }
        body.push_str("\n## Validation\n\n");
        for entry in &plan.validation {
            body.push_str(&format!(
                "- {}\n  Result: {}\n",
                entry.criterion,
                entry.result.as_deref().unwrap_or("pending")
            ));
        }
        for entry in &plan.decisions {
            body.push_str(&format!(
                "\n## Decision {}\n\n{}\n\nWhy: {}\n\nConsequences: {}\n",
                entry.id, entry.decision, entry.rationale, entry.consequences
            ));
        }
        body.push_str("\n## References\n\n");
        for reference in &plan.references {
            if !reference.contains('\n') && root_relative_reference(reference) {
                body.push_str(&format!("- [{reference}](../../{reference})\n"));
            } else {
                body.push_str(&format!("- {reference}\n"));
            }
        }
        for dependency in &plan.dependencies {
            body.push_str(&format!("- [Plan {dependency}]({dependency}.md)\n"));
        }
        if let Some(outcome) = &plan.outcome {
            body.push_str(&format!("\n## Outcome\n\n{outcome}\n"));
        }
        for lesson in &plan.lessons_learned {
            body.push_str(&format!("\n- Lesson: {lesson}\n"));
        }
        files.insert(format!("docs/plans/{}.md", plan.id), body);
    }
    let mut public = plans
        .iter()
        .filter(|p| p.visibility == Visibility::Public && p.status != Status::Cancelled)
        .filter_map(|p| p.roadmap.as_ref().filter(|r| r.enabled).map(|r| (p, r)))
        .collect::<Vec<_>>();
    public.sort_by_key(|(p, r)| (r.order, &p.id));
    let items = public
        .iter()
        .map(|(p, r)| PublicRoadmapItem {
            id: &r.id,
            plan_id: &p.id,
            name: &r.name,
            lede: &r.lede,
            version: r.target_version.as_deref(),
            status: &p.status,
        })
        .collect();
    let projection = PublicRoadmap {
        _generated: "lumilio-docgen plans generate; edit .agents/plans/*.json",
        items,
    };
    files.insert(
        "web/src/data/roadmap.generated.json".into(),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&projection).expect("serializable projection")
        ),
    );
    files
}

fn root_relative_reference(reference: &str) -> bool {
    [".agents/", "docs/", "crates/", "web/", "forks/", "assets/"]
        .iter()
        .any(|prefix| reference.starts_with(prefix))
}

pub fn run(root: &Path, command: Option<&str>) -> Result<(), String> {
    if !matches!(command, Some("validate" | "generate" | "check")) {
        return Err("usage: lumilio-docgen plans validate|generate|check".into());
    }
    let plans = load(root)?;
    if command == Some("validate") {
        return Ok(());
    }
    let files = generate(&plans);
    let directory = root.join("docs/plans");
    if directory.exists() {
        for entry in fs::read_dir(&directory).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            let relative = format!(
                "docs/plans/{}",
                path.file_name()
                    .ok_or("invalid filename")?
                    .to_string_lossy()
            );
            if path.extension().is_some_and(|e| e == "md") && !files.contains_key(&relative) {
                if command == Some("check") {
                    return Err(format!("orphan generated file: {relative}"));
                }
                fs::remove_file(path).map_err(|e| e.to_string())?;
            }
        }
    }
    for (relative, content) in files {
        let path = root.join(&relative);
        if command == Some("check") {
            if fs::read_to_string(path).ok().as_deref() != Some(&content) {
                return Err(format!("stale: {relative}; run just plans"));
            }
        } else {
            fs::create_dir_all(path.parent().ok_or("no parent")?).map_err(|e| e.to_string())?;
            fs::write(path, content).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
