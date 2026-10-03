# plans/ — Execution Plans

Every non-trivial change starts with a plan. Plans decompose work into independently verifiable units
so the agent can execute with a closed loop and a clear definition of done.

## Rules

- Filename: `NNNN-kebab-case-name.md` (e.g. `0001-offline-launch-slice.md`), number incremented per plan.
- Keep one plan focused on one deliverable; large efforts are split into multiple sequential plans,
  each mapped to a phase in `docs/roadmap.md`.
- Status is a single line at the top: `Status: proposed | in_progress | done | abandoned`.
- Update the status as you go. An `abandoned` plan states why.
- When a plan is done, tasks that changed architecture expectations are promoted to `docs/architecture.md`
  or a new ADR by the maintainer.

## Template

```markdown
# NNNN — <short goal>

- Status: proposed
- Phase: <roadmap phase>
- Author: <agent | maintainer>

## Goal

<One paragraph: what will be true when this plan is done.>

## Scope

- In scope: <bullet list>
- Out of scope: <bullet list — explicit exclusions prevent creep>

## Reference mapping

| LumilioCL module | Vendored reference (read-only) | What to extract (behavior only) |
|---|---|---|
| <crate::module> | <path under 3rd-party/, via docs/architecture.md> | <inputs/outputs/edge cases> |

## Tasks

- [ ] T1: <verifiable unit>
- [ ] T2: <verifiable unit>

## Validation

<Exact commands and acceptance criteria; core logic must have tests.>

## Risks / open questions

- <anything uncertain, to be logged in decisions if unresolved>
```

## After completion

- Mark `Status: done`.
- Once every task is checked and the Outcome is written, collapse the plan into one row of
  [`history.md`](history.md) and delete the file. Code comments and docs keep citing "plan NNNN";
  that row is what they resolve to. Plans with maintainer-only items left stay as files until those close.
