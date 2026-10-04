# 0021 — Lean harness: active plans, decision records, generated IA

- Status: accepted
- Date: 2026-10-03

## Context

The harness grew while the project expanded quickly: every idea was written down and none was retired. By October it had a product tree (`ARCH.md`) declared authoritative in six places, a 120-row table to register each upstream file before reading it, four flow-id namespaces (H-/L-/AC-/FLOW-REF-, about 230 ids) in `docs/workflows/`, 26 hand-written `docs/behavior/` files describing the current code, a storage plan in `docs/app-state.md`, a roadmap, and a "who is authoritative" table to navigate between them. The hand-written documents went stale as soon as the code changed. The scope rules also narrowed the thinking: ideas were judged against a draft tree instead of against what makes a better launcher. Once the IA was generated from code (ADR 0019), the code and the upstream source were the real facts, and the rest was friction.

## Decision

- **Facts live in code and upstream source.** The current product structure comes from the generated `docs/ia/paths/`. Upstream behavior comes from reading `3rd-party/` directly (HMCL, and Modrinth App per ADR 0022). Removed: `ARCH.md`, `docs/architecture.md`, `docs/roadmap.md`, `docs/workflows/`, `docs/behavior/`, `docs/ia/README.md`, and every "Behavior notes" pointer in code. `docs/app-state.md` is merged into ADR 0007.
- **No scope gate.** A feature does not need to fit a product tree or get an ADR before it is built. An ADR records a decision someone could later question, not permission.
- **The IA comment loses its flow-id field** (amends ADR 0019): `// ia[page]: 操作 | 层 / 组件 | 结果与反馈 [| 备注]`. Which upstream code inspired a change belongs to that change's plan, not to a permanent column.
- **Plans exist only while active.** `.agents/plans/` holds in-progress or proposed plans, named by slug, plus `backlog.md` for unplanned ideas. When a plan finishes, it is condensed into a decision record (context, decision, consequences, what shipped) under the next decision number. A plan with no lasting decision, such as a pure refactor, is deleted; git history keeps it. Code cites `ADR NNNN`, never a plan. Plans 0001–0038 are summarized in `decisions/plan-history.md`; the two still-active numbered plans (0030, 0031) keep their names.
- **Rules must earn their place.** A mistake is first answered with a test, lint or check. A prose rule is added only when it names the real failure it prevents.
- Housekeeping: the duplicate ADR 0013 (Microsoft sign-in) becomes 0020, and ADR 0008 is rejected.

## Consequences

- Positive: an agent reads AGENTS.md, the active plan and the generated IA and can start. No document claims something the code no longer does. New ideas, such as a plugin system, are judged on their merits.
- Negative / trade-offs: there is no written catalogue of upstream flows to check coverage against. Coverage questions are answered by reading HMCL and Modrinth at the time, which costs more per question. Rationale that existed only in `docs/behavior/` and is not in code comments, tests or ADRs is gone.
- Follow-ups: when a module's behavior is subtle enough to need prose, it goes in that module's doc comment.
