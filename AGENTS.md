# LumilioCL — Agent Work Manual

> This file is the entry point of the agent harness: hard rules, workflow, and commands only.
> Deep content is disclosed progressively via links — never paste large documents into the conversation.

## What this project is

LumilioCL is a Minecraft launcher written in **Rust + GPUI + gpui-component**, architected per `ARCH.md`
(Home / Library / Discover / Activity / Instance). Domain logic follows behavioral specifications and may adapt HMCL code (ADR 0011); `docs/architecture.md` is the single bridge to the vendored
reference implementation under `3rd-party/` (read-only; authoritative for behavior comparison and a permitted source of adapted code).

## Invariants (never violated)

1. **Derivation from `3rd-party/` is allowed (ADR 0011)**: the project will be released under AGPL and copying or
   adapting HMCL code is permitted. Keep attribution (source path and GPL-3.0 notice in a comment), respect files with
   a different license, and adapt to Rust and the crate boundaries rather than porting Java mechanically.
2. **`3rd-party/` is read-only**: never modify, move, or delete anything under it.
3. **ARCH.md is authoritative**: every page, navigation item, and feature must map to a node in `ARCH.md`.
   Out-of-scope features (e.g. 3D schematic viewer, telemetry) are not built by default; adding one requires an ADR first.
4. **Plan before executing**: non-trivial changes require writing/updating a plan under `.agents/plans/`
   (state machine in `.agents/plans/README.md`); architecture-affecting decisions require an ADR under `.agents/decisions/`.
5. **core does not depend on UI**: `lumilio-core` must never depend on gpui / gpui-component. Domain logic
   must stay independently testable and reusable — this is the only insurance against UI framework churn.
6. **Closed verification loop**: every change must pass all four checks below. Fix failures immediately;
   "commit now, fix later" is forbidden.

## Tech stack & structure

- Rust stable (pinned by `rust-toolchain.toml`), edition 2024.
- Workspace: `crates/lumilio-core` (domain logic), `crates/lumilio-ui` (GPUI), `crates/lumilio-app` (binary entry).
  Full mapping: `docs/architecture.md`.
- Async: core uses tokio; UI bridges via `cx.spawn`/channels. **No blocking I/O on the UI thread.**
- Before UI work, load the `gpui` and `gpui-component` skills; prefer existing gpui-component components over custom ones.
- Buttons, switches, tabs, segments and tags come from `lumilio-ui` (`key::Key`, `controls`, `kit`), drawn per
  design-language §12; do not reach for gpui-component's `Button`/`Switch`/`TabBar` in pages.
- UI look, motion, and copy follow `docs/design-language.md` (the world steps, the interface flows).
- Every page's information architecture — layers, components, user-reachable paths and their
  implementation status — lives in `docs/ia/`. Read the page's IA before any UI work; a feature that
  is not in the IA is not built, and a built feature is reflected there (update its status marker).

## Agent memories and skills

Recurring procedures live in `.agents/skills/`; read the relevant skill before
running its workflow. Current portable procedures are:

- `select-checks` — map a diff to the narrowest local checks, then run the full
  required verification loop before handoff;
- `exec-plan` — create and maintain the `.agents/plans/` state machine;
- `write-a-test` — choose the correct core/UI/app test boundary and prove a
  regression guard can fail;
- `postmortem` — record escaped failures and link the durable guardrails that
  prevent recurrence;
- `lumilio-motion-design` — apply `docs/design-language.md` to any UI,
  animation, copy, or world-scene change and review it visually.

Project-coupled decisions live in `.agents/decisions/`; escaped failures live
in `.agents/postmortems/`. Photos-specific procedures such as Go, React,
Taskfile, Lumen, or browser-E2E workflows are not part of LumilioCL.

## Verification commands (run in this order after every change)

```sh
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

## Workflow

1. Read this file, then `docs/architecture.md` (mapping tables) and `docs/roadmap.md` (phases) as needed.
2. At session start, read the current `in_progress` plan if one exists. For non-trivial tasks: create/update `.agents/plans/NNNN-*.md` (proposed → in_progress → done),
   decomposed into independently verifiable units.
3. Implement from the mapping tables. When consulting the vendored reference, only look at the files
   pointed to there, extract **behavior** (inputs/outputs/edge cases), and reimplement in your own words.
   For user-facing workflows, start at `docs/workflows/README.md`: identify the stable flow ID,
   scope, state ownership, and acceptance scenarios before implementation. Use the local mapped
   HMCL source as the behavioral baseline; distinguish current, target, and undecided behavior.
4. Run the four verification commands; add tests where needed (core logic must have tests).
5. Wrap up: update the plan status; append an ADR if architecture is affected; log open questions in decisions.

## Reference files (progressive disclosure)

- `docs/architecture.md` — target architecture and functional-domain mapping tables (read before implementing)
- `docs/workflows/README.md` — user journeys, HMCL behavior evidence, target business rules, and acceptance routing
- `docs/roadmap.md` — phased roadmap and exit criteria
- `docs/design-language.md` — visual, motion, and copy specification for all UI
- `docs/ia/` — per-page UI information architecture and shared patterns (version picker, content item, …)
- `.agents/plans/` — execution plans and template
- `.agents/decisions/` — architecture decision records (ADR) and template
- `.agents/skills/` — recurring procedures for planning, checks, and tests
- `.agents/postmortems/` — escaped failures and the guardrails that closed them

## Hardening principle

Every time the agent makes a mistake, harden the harness — add a rule, a template, a test, or a command —
instead of only correcting it verbally once.
