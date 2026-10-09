# cubiomes 1.21.5 → 26.3

- Status: in_progress

## Goal

Implement Minecraft Java releases 1.21.5 through 26.3 in the maintained cubiomes snapshot and the launcher adapter, with reproducible Mojang data and independent game goldens.

## References

- `world-explorer.md`, W14 and P8 T39–T44.
- Cubitect/cubiomes `e61f90580cbdd883214a8054670dacae655e59c0`, `biomenoise.c`, `tables/`, `finders.c` (MIT).
- Mojang version manifest, server data generators and bundled worldgen data.

## Tasks

- [x] Compare every release's generation inputs and record provenance.
- [x] Generate and validate biome search tables; adapt changed noise and structures.
- [x] Collect independent game goldens and verify C/Rust output.
- [x] Update candidate version identities, plugin integration and manual picker after automated verification.
- [x] Run focused checks and `just check`.
- [ ] Record maintainer game verification before merging/publishing verified support.

## Validation

Baseline 1.21.4 biome and structure coordinates must remain unchanged. New versions must match original game samples across three seeds, dimensions and heights, including changed biomes and structure positions. Unknown versions remain rejected.

- Original release runtimes: 44,928 biome samples across 13 releases match;
  15,375 regional placement samples across 12 new releases match.
- The encoder reproduces all 9,112 numeric words of `btree21wd.h`. Instead of
  independently reconstructing Mojang's RTree ordering from parameter reports,
  it encodes the original runtime's numeric tree. This preserves tie-breaking
  and 26.3's irregular branches. Reports and resource diffs remain independent
  inputs for auditing changes.
- Replacing 26.3's table with 26.2's intentionally fails on seed 262,
  quart (-1549, -12, -3807): plains=1 instead of dappled_forest=188.
  Restoring the table passes the same focused test.
- All 39 original vanilla worlds were generated and read: 706 saved biome
  samples, 159 `/locate` results (including camp grids) and 39 bounded spawn
  estimates pass. Full `just check` passes (build, workspace tests, Clippy with
  warnings denied, fmt), using `CARGO_INCREMENTAL=0 DISPLAY=:99` in the managed
  Linux environment. `git diff --check` also passes.
- Camp grids are exact; surface-height/template viability remains estimated
  and is marked approximate in the adapter and map layer.
- Saved vanilla spawn differs from the upstream terrain approximation by
  20 blocks for baseline seed -123456789012345. Spawn is now marked estimated
  for every release; saved-world checks retain a bounded-position comparison.
- Full checks exposed two baseline build/check issues: the block-colour
  selector returned a reference to a cloned temporary (fixed without changing
  selection), and backup collision timestamps could cause retention to delete
  the newest copy. A deterministic future-stamp regression test fails before
  the monotonic timestamp fix and passes afterwards.
- Build caches exhausted the environment disk during validation. Rebuildable
  incremental caches were removed, affected disposable worlds reset, and the
  full check is rerun with `CARGO_INCREMENTAL=0`. Committed data is unchanged.

## Open questions

- Human F3 and `/locate` verification required by W14 is pending. Candidate
  version-table changes are prepared for review; maintainer acceptance is
  required before merging/publishing them as verified support. No human
  acceptance date is claimed.
