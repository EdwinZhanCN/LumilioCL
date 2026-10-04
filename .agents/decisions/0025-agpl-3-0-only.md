# 0025 — The project is AGPL-3.0-only

- Status: accepted
- Date: 2026-10-04

## Context

ADR 0011 committed to releasing under AGPL and left adding the `LICENSE` file and metadata as a follow-up. Since then, code has been adapted from two upstreams with different terms: HMCL is GPL-3.0-or-later, and Modrinth App (`apps/app`, `apps/app-frontend`, `packages/app-lib`, `packages/ui`) is GPL-3.0-only (ADR 0022). GPLv3 §13 allows either to be combined with an AGPLv3 work.

## Decision

- The repository is licensed **AGPL-3.0-only**. The text is in `LICENSE` (the canonical GNU text, SHA-256 `8486a10c…07ef`), every crate declares `license.workspace = true`, the README has a License section, and Settings → About shows it.
- We choose "only" rather than "or later" because part of the combined work is GPL-3.0-only Modrinth code, which can never move to a later version. "Or later" would promise something the whole program cannot deliver.
- Attribution comments name each upstream's own license, not ours: HMCL code says GPL-3.0-or-later with its copyright line, and Modrinth code says GPL-3.0-only. `crates/lumilio-docgen/tests/attribution.rs` enforces this, after five HMCL attributions had been written as "AGPL-3.0" without a copyright line.

## Consequences

- Positive: the license is explicit before any release, and attribution mistakes fail `cargo test`.
- Negative / trade-offs: moving to a later AGPL version would need every contributor's agreement, and the Modrinth-derived parts could not move at all.
- Follow-ups: per-file license headers are not required and are not added. Revisit if contributors outside the maintainer appear.
