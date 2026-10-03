# .agents — Agent Harness Mechanics

This directory is the state layer of the LumilioCL agent harness: **plans** (execution state), **decisions** (durable memory), **postmortems** (escaped failures), and **skills** (repeatable procedures).
Design principles follow the 2026 harness-engineering consensus: *an agent is the model plus everything built around it*;
the scaffolding is a real artifact that tightens every time the agent slips; quality is enforced with **mechanisms, not prompts**.

## File map

| Path | Purpose | Written by |
|------|---------|-----------|
| `AGENTS.md` (repo root) | Hard rules, workflow, verification commands. Facts only, no tutorials | Maintainers |
| `.agents/plans/NNNN-*.md` | Executable plans: task breakdown, validation criteria, state machine | Agent (every non-trivial task) |
| `.agents/decisions/NNNN-*.md` | ADRs: architecture decisions, constraints, rejected alternatives | Agent + maintainer |
| `.agents/postmortems/NNNN-*.md` | Escaped failures and the guardrails that stopped recurrence | Agent + maintainer |
| `.agents/skills/*/SKILL.md` | End-to-end recurring procedures with validation and failure modes | Maintainers + agents |
| `docs/architecture.md` | Stable target architecture and mapping tables (where plans/ADRs settle) | Maintainer review |

## Lifecycles

### plans/ — plan-then-execute

1. Start: create `plans/NNNN-kebab-name.md` (incrementing number) or update the current `in_progress` plan.
2. A plan must contain: goal, scope & out-of-scope, reference mapping (pointing at the mapping tables in
   `docs/architecture.md`), a task breakdown where each item is independently verifiable, and validation criteria.
3. State machine: `proposed → in_progress → done` (or `abandoned` with a reason).
4. On completion: mark `done`. Milestone-spanning plans are registered as phases in `docs/roadmap.md`.

### decisions/ — append-only ADR log

- Decisions affecting architecture, dependencies, licensing, or scope MUST be recorded as ADRs
  (template in `decisions/README.md`).
- Accepted ADRs are **never edited**; supersede them with a new ADR. This keeps memory a git-log-like
  history of *why*, not a rewrite of *what*.
- Numbering starts at `0001` and increments.

### postmortems/ — escaped-failure memory

Create a postmortem when a defect reaches a user, release, or protected branch
and the interesting question is why the existing safety nets missed it. The
record must link the regression test, rule, or check that now prevents the same
escape. An empty directory is healthy.

### skills/ — procedure ownership

Each skill owns one recurring workflow: when to invoke it, exact steps,
validation, and known failure modes. Root rules and architecture docs state
what must hold; skills state how to perform the repeated procedure. Do not add
Photos-specific procedures that have no LumilioCL equivalent.

## Context budget (progressive disclosure)

- `AGENTS.md` and plan files stay lean (hard facts only). Details live in linked files — **link, don't duplicate**.
- The agent should read a file fully only when the task requires it; prefer targeted reads (line ranges, grep).
- Do not echo large reference content into the conversation; summarize in your own words.
- Structured text (tables, checklists, command blocks) beats prose for machine consumption.

## Hardening loop

When the agent (or a reviewer) finds a recurring mistake:

1. Fix the immediate issue.
2. Add the cheapest mechanism that prevents recurrence, in this order:
   rule in `AGENTS.md` → template/checklist in `.agents/` → unit/integration test → lint/CI command.
3. If the mistake reveals a design constraint, record it as an ADR.

## Delegation guidance

- Delegate to sub-agents for: isolated research on vendored-reference behavior (via the mapping tables),
  parallel file-gathering, review of a fresh perspective, or long-running verification whose logs would flood context.
- Give sub-agents concrete paths and constraints; ask for a concise final message with failing diagnostics included.
- Disjoint write scopes when multiple agents edit files in parallel.

## Session continuity

- The current `in_progress` plan is the session state. At session start, read it first;
  at session end, leave it accurate (status, remaining tasks, open questions).
- Do not keep conversation-only state; anything worth remembering belongs in plans or decisions.
- A non-trivial change must update one durable memory in the same change: its
  active plan, an ADR, or a postmortem. Mechanical/local edits are exempt.
