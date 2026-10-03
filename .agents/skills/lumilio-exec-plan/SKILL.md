---
name: lumilio-exec-plan
description: Use for every non-trivial LumilioCL change — create or update a
  plan under .agents/plans, maintain its state machine, and close it with
  evidence and durable follow-ups.
---

# Execution Plans

Plans are executable session state, not a second ADR archive. LumilioCL keeps
completed plans as history under `.agents/plans/`; durable architectural why
belongs in `.agents/decisions/`.

## When a plan is required

Create or update a plan before implementation when work spans multiple files,
changes architecture or dependencies, consults the read-only reference, adds
a page or workflow, or needs more than one independently verifiable unit. A
mechanical/local edit may skip a plan.

## Procedure

1. Read the current `in_progress` plan. Continue it when the scope matches;
   otherwise create the next `NNNN-kebab-case-name.md`.
2. Start at `Status: proposed`, then move to `in_progress` before the first
   implementation write. Keep one focused goal, explicit out-of-scope items,
   mapping tables, independently verifiable tasks, validation commands, and
   open questions.
3. Update the plan in the same change that alters the implementation scope or
   a frozen contract. Keep task checkboxes honest.
4. Before handoff, run the validation boundaries, record failures or
   environment blockers, and mark `Status: done` only when no required work
   remains. Use `abandoned` with a reason when the direction is rejected or
   superseded.
5. Promote lasting architecture, dependency, licensing, or scope decisions to
   a numbered ADR. Do not delete the completed plan; it is the execution log.

## Required shape

Use the template in `.agents/plans/README.md`: goal, scope/out-of-scope,
reference mapping, tasks, validation, and risks/open questions. Each task must
have a concrete observation that can prove it complete.
