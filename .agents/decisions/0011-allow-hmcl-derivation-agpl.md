# 0011 — Allow HMCL-derived code; project to be released under AGPL

- Status: accepted
- Date: 2026-09-30

## Context

Invariant 1 of `AGENTS.md` forbade copying or deriving anything from `3rd-party/` and required behavior-only reimplementation. The maintainer has decided the project will be open-sourced under AGPL (AGPL-3.0) and explicitly permits copying from the vendored HMCL. HMCL is licensed GPL-3.0 (`3rd-party/HMCL/LICENSE`); GPL-3.0 and AGPL-3.0 permit combining works, with the combined work governed by AGPL for the network-use terms.

## Decision

Code, identifiers, strings, algorithms and structure may be taken or adapted from `3rd-party/HMCL` into `crates/`. The vendored tree stays read-only (invariant 2). The reference mapping table in `docs/architecture.md` stays as the routing aid for where to look, no longer as a clean-room barrier.

Obligations that come with this:

- Keep attribution: a file or function adapted from HMCL carries a comment naming the HMCL source path and its GPL-3.0 copyright (taken from that file's header). Do not strip existing notices.
- Only HMCL's own code is covered. Files that state a different license, bundled third-party libraries under `3rd-party/HMCL/lib`, and Minecraft/Mojang assets keep their own terms; check the file header before copying.
- Java idioms are not ported mechanically; adapt to Rust, async and the crate boundaries (core must not depend on UI).
- Adding the AGPL `LICENSE` file, `license` fields and per-crate notices is a separate release-preparation task, not done by this ADR.

## Consequences

- Positive: faster, more faithful parity with proven launcher behavior and edge cases.
- Negative / trade-offs: the repository becomes a derivative work and must be distributed under AGPL-compatible terms; provenance of copied code must be tracked; previous "zero derived content" reviews no longer apply.
- Follow-ups: add LICENSE and metadata before any public release; keep a note of copied origins in the mapping table or file comments.
