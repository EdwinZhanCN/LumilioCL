//! Read-only plan queries and dispatch to the existing projection commands.

use std::path::Path;

use lumilio_docgen::plans;
use lumilio_docgen::plans::{Plan, Status};
use serde::Serialize;

pub(super) const USAGE: &str = "usage: lumilio-docgen plans validate|generate|check
       lumilio-docgen plans list [--status STATUS] [--json]
       lumilio-docgen plans show <id> [--json]
STATUS: proposed, in_progress, blocked, completed, cancelled, incomplete
incomplete includes proposed, in_progress and blocked (not cancelled).";

pub(super) fn run(root: &Path, args: &[String]) -> Result<String, String> {
    match args.first().map(String::as_str) {
        Some("--help" | "-h") if args.len() == 1 => Ok(format!("{USAGE}\n")),
        Some("validate" | "generate" | "check") if args.len() == 1 => {
            plans::run(root, Some(&args[0]))?;
            Ok(String::new())
        }
        Some("list" | "show") if args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h") => {
            Ok(format!("{USAGE}\n"))
        }
        Some("list") => list(root, &args[1..]),
        Some("show") => show(root, &args[1..]),
        _ => Err(USAGE.into()),
    }
}

fn status_name(status: &Status) -> &'static str {
    match status {
        Status::Proposed => "proposed",
        Status::InProgress => "in_progress",
        Status::Blocked => "blocked",
        Status::Completed => "completed",
        Status::Cancelled => "cancelled",
    }
}

fn matches_status(plan: &Plan, filter: Option<&str>) -> bool {
    match filter {
        None => true,
        Some("incomplete") => matches!(
            plan.status,
            Status::Proposed | Status::InProgress | Status::Blocked
        ),
        Some(status) => status_name(&plan.status) == status,
    }
}

#[derive(Serialize)]
struct TaskCounts {
    completed: usize,
    cancelled: usize,
    total: usize,
}

#[derive(Serialize)]
struct Summary<'a> {
    id: &'a str,
    title: &'a str,
    status: &'a Status,
    tasks: TaskCounts,
}

fn list(root: &Path, args: &[String]) -> Result<String, String> {
    let mut json = false;
    let mut filter = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" if !json => json = true,
            "--status" if filter.is_none() => {
                let status = args.next().ok_or("--status requires a value")?;
                if !matches!(
                    status.as_str(),
                    "proposed"
                        | "in_progress"
                        | "blocked"
                        | "completed"
                        | "cancelled"
                        | "incomplete"
                ) {
                    return Err(format!("unknown status: {status}\n{USAGE}"));
                }
                filter = Some(status.as_str());
            }
            _ => return Err(format!("unexpected or repeated argument: {arg}\n{USAGE}")),
        }
    }
    let plans = plans::load(root)?;
    let rows: Vec<_> = plans
        .iter()
        .filter(|plan| matches_status(plan, filter))
        .map(|plan| Summary {
            id: &plan.id,
            title: &plan.title,
            status: &plan.status,
            tasks: TaskCounts {
                completed: plan
                    .tasks
                    .iter()
                    .filter(|t| t.status == Status::Completed)
                    .count(),
                cancelled: plan
                    .tasks
                    .iter()
                    .filter(|t| t.status == Status::Cancelled)
                    .count(),
                total: plan.tasks.len(),
            },
        })
        .collect();
    if json {
        return as_json(&rows);
    }
    if rows.is_empty() {
        return Ok("No matching plans.\n".into());
    }
    let mut output = "ID\tSTATUS\tTASKS\tTITLE\n".to_owned();
    for row in rows {
        let cancelled = if row.tasks.cancelled == 0 {
            String::new()
        } else {
            format!(" (+{} cancelled)", row.tasks.cancelled)
        };
        output.push_str(&format!(
            "{}\t{}\t{}/{}{cancelled}\t{}\n",
            row.id,
            status_name(row.status),
            row.tasks.completed,
            row.tasks.total,
            row.title.split_whitespace().collect::<Vec<_>>().join(" ")
        ));
    }
    Ok(output)
}

fn show(root: &Path, args: &[String]) -> Result<String, String> {
    let mut json = false;
    let mut id = None;
    for arg in args {
        match arg.as_str() {
            "--json" if !json => json = true,
            value if !value.starts_with('-') && id.is_none() => id = Some(value),
            _ => return Err(format!("unexpected or repeated argument: {arg}\n{USAGE}")),
        }
    }
    let id = id.ok_or_else(|| format!("show requires a plan id\n{USAGE}"))?;
    let plans = plans::load(root)?;
    let plan = plans
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("plan not found: {id}"))?;
    if json {
        return as_json(plan);
    }
    let mut output = format!(
        "# {} — {}\n\nStatus: {}\n\n{}\n\nTasks:\n",
        plan.id,
        plan.title,
        status_name(&plan.status),
        plan.summary
    );
    for task in &plan.tasks {
        output.push_str(&format!(
            "- {} [{}] {}\n  Acceptance: {}\n",
            task.id,
            status_name(&task.status),
            task.title,
            task.acceptance
        ));
    }
    output.push_str("\nValidation:\n");
    for validation in &plan.validation {
        output.push_str(&format!(
            "- {}\n  Result: {}\n",
            validation.criterion,
            validation.result.as_deref().unwrap_or("pending")
        ));
    }
    if let Some(outcome) = &plan.outcome {
        output.push_str(&format!("\nOutcome: {outcome}\n"));
    }
    Ok(output)
}

fn as_json(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_string_pretty(value)
        .map(|output| format!("{output}\n"))
        .map_err(|error| error.to_string())
}
