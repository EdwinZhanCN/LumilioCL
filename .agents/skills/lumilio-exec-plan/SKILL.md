---
name: lumilio-exec-plan
description: Use for multi-step or multi-session LumilioCL work — create or
  continue a plan under .agents/plans, keep it honest, and on completion
  condense it into a decision record or delete it.
---

# Execution Plans

A plan is session state for work in flight. It exists only while the work is
active (ADR 0021). Durable "why" goes to `.agents/decisions/`.

## When a plan is needed

The work spans several verifiable steps or more than one session, or it makes
a choice worth recording. A local or mechanical edit needs no plan.

## Procedure

1. Read the `in_progress` plans in `.agents/plans/`. Continue the one that
   matches; otherwise create `<slug>.md` from the template in
   `.agents/plans/README.md`. If the idea is in `backlog.md`, remove it there.
2. Move to `Status: in_progress` before the first implementation write.
3. As you go, tick tasks, name the upstream files you drew on
   (`3rd-party/HMCL/…`, `3rd-party/modrinth/…`), and record surprises.
4. Before handoff, run the four verification commands. Record failures or
   environment blockers in the plan; don't hide them.
5. When the work is done:
   - If it made a choice someone could later question, write the next
     numbered decision record: context, decision, consequences, and a line on
     what shipped. Code cites that ADR number.
   - Delete the plan file in the same change.
   - Items only the maintainer can do (a real sign-in, a visual review) keep
     the plan open, or move them to `backlog.md` under "只有维护者能做".

## Failure modes

- **Keeping a finished plan "for history".** Git has the history; the decision
  record has the why.
- **Citing a plan from code.** Plans disappear; cite the ADR.
