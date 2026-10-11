---
name: lumilio-exec-plan
description: Use for multi-step or multi-session LumilioCL work — create or continue a JSON plan, track acceptance and decisions, generate projections, and retain the completed record.
---

# Retained execution plans

JSON in `.agents/plans/` is the only editable lifecycle source. Generated
Markdown and website data are projections. Small local fixes need no plan.

1. Read matching `in_progress` JSON plans and continue the relevant one.
   Otherwise create `<slug>.json` using `.agents/schemas/examples/proposed.json`
   and `.agents/plans/README.md`. Assign stable plan/task IDs. Remove a backlog
   idea when it becomes a plan.
2. Set `status: in_progress` before implementation. Record scope and each
   task's observable acceptance. Default to `visibility: internal`; explicitly
   curated public fields belong in `roadmap`.
3. Update task status, validation evidence, decisions with rationale and
   consequences, upstream references and lessons. Record real blockers in
   summary/validation; never claim completed work while acceptance is pending.
4. Run `cargo run -p lumilio-docgen -- plans validate`, then `just plans`.
   Use `lumilio-select-checks`: hand off code after `just check`, docs after
   `just docs`, website changes after `just web`.
5. Add validation results and `outcome`, mark tasks terminal and plan
   `completed`, regenerate, then run `just plans-check`. Keep JSON. If real
   sign-in, release or visual acceptance remains, keep the plan open with an
   explicit remaining task. Cancelled plans are also retained.

Ordinary decisions and postmortems go in `decisions` and `lessonsLearned`.
Cross-plan contracts live in code/API/design
documents and may cite stable plan IDs or document paths. Never generate an
implemented IA path from roadmap. Completed is not released; use actual Releases.

Do not create standalone ADRs, postmortems, legacy/archive directories, old
Markdown redirects or historical mapping tables. Consult Git history when needed. Never
publish full plans as website data: the generator uses a field whitelist.
