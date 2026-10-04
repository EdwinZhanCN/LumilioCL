# 0022 — Modrinth App as a second upstream reference

- Status: accepted
- Date: 2026-10-03

## Context

HMCL (ADR 0011) is the reference for feature coverage and edge cases, but it is Java and JavaFX. Modrinth App is an open-source launcher whose backend, `packages/app-lib` (formerly "theseus"), is Rust. It covers the same ground as `lumilio-core`: instances and profiles, modpack install, Java runtimes, Minecraft auth, worlds, processes and logs, version metadata (`packages/daedalus`). The maintainer wants both projects as references.

## Decision

- Vendor a shallow clone of `github.com/modrinth/code` at `3rd-party/modrinth` (first clone: `71f35eb`, 2026-10-02). Like `3rd-party/HMCL`, it is git-ignored and read-only.
- **Licensing.** `apps/app` and `packages/app-lib` are GPL-3.0-only (their `COPYING.md`/`LICENSE`). GPLv3 §13 lets them combine with this AGPL-3.0 project, so code may be adapted under the same obligations as ADR 0011: a comment naming the source path and the GPL-3.0 notice, and adaptation to our crate boundaries rather than mechanical copying. Other packages in the monorepo carry their own licenses (e.g. `packages/daedalus` is MIT; `apps/labrinth` is the server): check each package's `LICENSE`/`COPYING.md` before taking anything.
- **Branding is excluded.** Modrinth's logos, cover images and other branding assets are all rights reserved (`COPYING.md`) and must never be copied.
- **Which reference for what:** Modrinth App first for how something is built in Rust (async structure, metadata handling, auth, process supervision). HMCL first for breadth of features, platform quirks and edge cases. Neither decides our structure, naming, copy or UI.

## Consequences

- Positive: a working Rust implementation to adapt from, not only Java to translate; two independent references for contested behavior.
- Negative / trade-offs: another 400+ MB local checkout; Modrinth's app-lib is built around Tauri and its own state model, so pieces need untangling before reuse.
- Follow-ups: refresh the clone deliberately and note the new commit in the plan that needed it.
