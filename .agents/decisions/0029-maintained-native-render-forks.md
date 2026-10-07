# 0029 — Maintain the Rust schematic rendering chain in the repository

- Status: accepted
- Date: 2026-10-05

The rendering fixes this record left for later (glass sorting, 26.x model compatibility)
are decided in ADR 0034.

## Context

ADR 0028 introduced a native preview using Nucleation and Schematic-Mesher.
The latest pinned upstream GPU renderer lacks camera-dependent transparent
sorting, while the mesher cannot parse some Minecraft 26.2 element rotations
and still refers to old sign texture layouts. The maintainer wants to fix
native rendering without waiting for upstream releases.

The current schemat.io interactive viewer uses a separate browser renderer
with transparent sorting and a prebuilt resource pack. Sharing Nucleation does
not make its rendering behavior identical to the native GPU renderer.

## Decision

Maintain editable source snapshots of Nucleation and Schematic-Mesher under
`forks/`, based on the exact commits previously selected by Cargo.lock. Keep
upstream notices, source modules, build data and supporting Rust crates/tests.
Omit generated bindings and unrelated website/packaging trees. `3rd-party/`
continues to contain read-only references.

The adapter depends on the local Nucleation path; Nucleation depends on the
adjacent mesher path, and the mesher's development dependency points back to
local Nucleation. The fork packages remain outside the launcher workspace's
membership to avoid running unrelated upstream tools and feature suites in
every launcher check. Changes to exercised code must have focused regression
coverage and pass the launcher `just check` gate.

Baseline commits, local changes and upstream synchronization procedure live
in `forks/README.md`. This is an in-repository fork; it does not create external
GitHub repositories. The browser renderer is not imported. Game textures still
come from the instance JAR, never from the website's prebuilt pack.

## Consequences

We can change native GPU drawing, model parsing and meshing in one reviewable
repository, at the cost of owning regression fixes and upstream synchronization.
Nucleation remains MIT; Schematic-Mesher remains AGPL-3.0-only. The adoption
changes dependency ownership, not rendering behavior. Glass sorting and 26.2
resource compatibility remain subsequent work.

## What shipped

Both source snapshots and local dependency wiring are present in the main
worktree. Their `src/` trees match the recorded upstream baselines exactly;
adoption changes only dependency manifests. Metadata and dependency-tree
inspection confirm local paths for both libraries. The focused renderer suite
passed (7 tests, including actual GPU pixels; performance test ignored), and
the full launcher `just check` passed on macOS. No rendering algorithm fix is
claimed by this adoption.
