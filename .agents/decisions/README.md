# decisions/ — Architecture Decision Records (ADR)

ADR log is the project's durable memory of *why things are the way they are*.
Append-only: accepted records are never rewritten; supersession happens via a new ADR.

## When to write an ADR

- New dependency or removal of one.
- Change in architecture (module boundaries, layering, async model, storage format).
- Licensing or scope decisions (e.g. adding a feature not in `ARCH.md`).
- Any constraint the team wants to outlive the current conversation.

## Rules

- Filename: `NNNN-kebab-case-title.md`, numbering starts at `0001`, increments by one.
- Status: `proposed | accepted | superseded-by-NNNN | rejected`.
- Context and consequences are written in the team's words — never copy from the vendored reference.

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
