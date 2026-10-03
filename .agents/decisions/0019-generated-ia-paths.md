# 0019 — IA user paths are generated from annotated code

- Status: accepted
- Date: 2026-10-03

## Context

`docs/ia/` lists every user path (action → layer/component → result → flow id → status) by hand. The status column drifts: a feature lands or is removed and the table keeps saying the old thing. Reconciling the tables after plans 0022–0036 corrected several "not built" rows that were built.

## Decision

A path that is built carries a one-line comment at the code that implements it: `// ia[page]: 操作 | 层 / 组件 | 结果与反馈 | 编号 [| 备注]`. A std-only tool crate, `lumilio-docgen`, scans `crates/*/src`, validates the flow ids against `docs/workflows/`, and writes `docs/ia/paths/<page>.md`. Status is not written by anyone: a path exists in the generated file if and only if its annotation exists, and every generated row means built. Handwritten per-page IA documents are retired: what is not built or out of scope is a short table in `docs/ia/README.md`, shared interaction patterns live in `docs/design-patterns.md`, and layout and look stay in `docs/design-language.md`. A test in the crate fails when the generated files are stale, so `cargo test` keeps them current.

## Consequences

- Positive: the built-paths table cannot claim something the code no longer has; deleting a feature deletes its row with it; a mistyped flow id fails the test.
- Negative / trade-offs: the comment can still lie about what the code does (it proves a path was claimed in that file, not that it works); a new workspace member; annotations add noise next to UI code.
- Follow-ups: pages are migrated one at a time, comparing the generated table with the handwritten one before the handwritten rows are removed; files are split per page as each is migrated.
