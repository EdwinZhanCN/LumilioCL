# decisions/ — Architecture Decision Records (ADR)

ADR log is the project's durable memory of *why things are the way they are*.
Append-only: accepted records are never rewritten; supersession happens via a new ADR.

## When to write an ADR

- A finished plan that made a choice someone could later question (ADR 0021):
  condense it here instead of keeping the plan.
- A new dependency, or removing one; a change to module boundaries, layering,
  the async model or a storage format; licensing.
- Any constraint that should outlive the current conversation.

An ADR records a decision, not permission: nothing needs an ADR *before* it can
be built.

## Rules

- Filename: `NNNN-kebab-case-title.md`, numbering starts at `0001`, increments by one.
- Status: `proposed | accepted | superseded-by-NNNN | rejected`.
- Context and consequences are written in the team's words.
- `plan-history.md` is the one unnumbered record: what plans 0001–0038 delivered.

## Template

```markdown
# NNNN — <Title>

- Status: proposed | accepted | superseded-by-NNNN | rejected
- Date: <YYYY-MM-DD>

## Context

<What problem or constraint forces this decision? Facts only.>

## Decision

<What we decided, in one or two sentences.>

## Consequences

- Positive: <...>
- Negative / trade-offs: <...>
- Follow-ups: <...>
```

## Status transitions

- `proposed` → `accepted` when maintainer agrees.
- `accepted` → `superseded-by-NNNN` when a later ADR changes the decision (the old ADR stays as history).
- `proposed` → `rejected` when explicitly declined (keep the record: rejected alternatives are valuable context).
