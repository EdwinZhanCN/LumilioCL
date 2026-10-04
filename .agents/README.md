# .agents — Harness state

| Path | Holds |
|---|---|
| `plans/` | Active and proposed plans, plus `backlog.md`. Finished plans leave (ADR 0021) |
| `decisions/` | Decision records (ADRs), including condensed finished plans and `plan-history.md` |
| `postmortems/` | Escaped failures and the test or check that now prevents them |
| `skills/` | Recurring procedures: when to use them, exact steps, failure modes |

## Rules of thumb

- **Session state is the active plan.** Read it at the start and leave it accurate at the end.
  Conversation-only state is lost.
- **Facts come from code, not prose.** The product structure is the generated `docs/ia/paths/`;
  upstream behavior is `3rd-party/` itself. Don't write documents that restate either.
- **Harden with mechanisms.** When a mistake recurs, prefer, in this order: a test → a lint or
  check → a skill step → a line in `AGENTS.md`. A prose rule must name the failure it prevents.
  Delete rules that no longer prevent anything.
- **Delegation.** Give sub-agents concrete paths and disjoint write scopes, and ask them for a
  short final report.
