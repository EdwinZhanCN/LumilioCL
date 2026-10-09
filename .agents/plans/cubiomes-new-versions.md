# cubiomes 1.21.5 → 26.3

- Status: in_progress

## Goal

Implement Minecraft Java releases 1.21.5 through 26.3 in the maintained cubiomes snapshot and the launcher adapter, with reproducible Mojang data and independent game goldens.

## Scope

- In：在 `forks/cubiomes` 里补新版本（1.21.5 … 26.x）的世界生成，首个目标 26.3；`cargo xtask worldgen-diff`、`worldgen-golden`、`cubiomes-btree` 三个开发工具（不进发布包）；金样与对比测试；`forks/cubiomes/LUMILIO.md` 的逐条记录。
- Out：World Explorer 的功能阶段（P0–P7，见 [`world-explorer.md`](world-explorer.md)）；把版本表里没有的版本映射到相邻或最新的生成实现（W14）。

本计划从 [`world-explorer.md`](world-explorer.md) 的延后阶段 P8 拆出（2026-10-09），任务编号 T39–T44 保持不变。它是可选的，不阻塞 World Explorer 收尾（T45）。

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

## 风险

- 自维护 fork 的长期成本：Mojang 每个版本都可能改世界生成（新群系、参数表、噪声、结构配置），每次都要按 W14 跑一遍差异、改 C、采金样、实机核对。P8 是第一次，工作量最难估；之后每个大版本都要重复。
- 自己实现可能出错：多噪声树的构造、新群系的参数或结构规则理解错，地图就会画错而看起来很真。兜底是 W14 的金样（我们自己从真实世界测得）和维护者实机核对；金样只覆盖采样点，采样之外的错误仍可能漏掉，所以每个版本至少三个种子、三个维度、多个高度。
- 空窗期：一个新版本发布后，到我们完成五步之前，种子底图和结构 Overlay 在这个版本上显示「这一版还不能查」。玩家升级游戏后会先看到它。
- 上游 cubiomes 若恢复更新：要把上游的提交合进 fork，并逐个判断本地补丁是否被覆盖。两边对同一版本的实现不同时，以金样和实机为准（W14）；新群系的 id 若和我们自己分配的不同，要迁移种子瓦片缓存（缓存键含配色和版本，换 id 时清掉对应缓存）。

## Open questions

- Human F3 and `/locate` verification required by W14 is pending. Candidate
  version-table changes are prepared for review; maintainer acceptance is
  required before merging/publishing them as verified support. No human
  acceptance date is claimed.

## 实施记录

- 2026-10-09：从 `world-explorer.md` 的 P8 拆出成独立计划，T39–T44 原文搬入、编号不变；维护者正在推进。
- 2026-10-09：与维护者分支 `codex/cubiomes-1.21.5-26.3`（PR #10）合并：任务、验证和开放问题以维护者的版本为准；保留拆分说明、Scope 和从 `world-explorer.md` 搬来的风险。1.21.5–26.3 已实现并通过自动验证，等待维护者实机核对。
