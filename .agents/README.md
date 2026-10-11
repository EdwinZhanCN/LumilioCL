# .agents — Harness state

| Path | Holds |
|---|---|
| `plans/` | Authoritative JSON lifecycle, including completed/cancelled plans; `backlog.md` holds ideas |
| `schemas/` | Versioned structural contract and example; docgen enforces semantic rules |
| `skills/` | Recurring procedures: when to use them, exact steps, failure modes |

## Rules of thumb

- **Session state is the active plan.** Read it at the start and leave it accurate at the end.
  Conversation-only state is lost.
- **Edit JSON only.** Follow [the lifecycle](plans/README.md); `just plans`
  generates Markdown and website roadmap, `just docs` verifies it. New decisions
  and lessons live with the retained plan.
- **One record system.** No standalone ADRs, postmortems, legacy/archive directories or
  historical mapping tables. Consult Git history for removed material.
- **Facts come from code, not prose.** The product structure is the generated `docs/ia/paths/`;
  upstream behavior comes from its published source. Don't write documents that restate either.
- **Harden with mechanisms.** When a mistake recurs, prefer, in this order: a test → a lint or
  check → a skill step → a line in `AGENTS.md`. A prose rule must name the failure it prevents.
  Delete rules that no longer prevent anything.
- **Delegation.** Give sub-agents concrete paths and disjoint write scopes, and ask them for a
  short final report.
