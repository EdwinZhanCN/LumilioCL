# plans/ — Active plans

Only work that is in progress or proposed lives here, plus [`backlog.md`](backlog.md)
for ideas nobody has planned yet. Finished work leaves this folder (ADR 0021).

## Rules

- Filename: a slug, `kebab-case-goal.md`. No number: a plan gets a decision number
  only if it becomes a decision record. The legacy plans `0030-…` and `0031-…`
  keep their names until they close.
- Write a plan for work that spans several steps or sessions. A local edit needs none.
- Status is one line at the top: `proposed | in_progress`.
- Keep it honest as you go: tick tasks, record what you learned, note open questions.
- When upstream code informs the work, name the files (e.g.
  `3rd-party/modrinth/packages/app-lib/src/api/jre.rs`) under **References**.

## When it finishes

1. If it contains a decision someone could later question, condense it into the next
   `.agents/decisions/NNNN-*.md`: context, decision, consequences, and one line on
   what shipped. Drop the task list and validation log.
2. Delete the plan file. A pure refactor or a mechanical change just gets deleted.
3. If it was abandoned, delete it too. Write a `rejected` decision if the reason
   would save someone from trying again.

## Template

```markdown
# <short goal>

- Status: proposed

## Goal

<One paragraph: what will be true when this is done.>

## Scope

- In: …
- Out: …

## References

- <upstream files, ADRs>

## Tasks

- [ ] T1: <verifiable unit>

## Validation

<Commands and acceptance; core logic has tests.>

## Open questions

- …
```
