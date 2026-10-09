# 地图结构图标换成 MinecraftSearch，并开放更多结构

- Status: in_progress

## Goal

地图的结构图标整批换成 MinecraftSearch `images/structures/` 的资源（替换原来的
Axolotl 转存那份），声明写清真正的来源；同时把 cubiomes 已经支持、但世界地图还没
开放的结构种类接上，让它们能在图上显示（`desert_well`、`mineshaft`、
`buried_treasure`、`amethyst_geode`、`end_gateway`，以及给 26.3 的废弃营地一个真图标）。

## Scope

- In：`crates/lumilio-ui/assets/map-icons/` 资源与其 `NOTICE.md`、`ATTRIBUTIONS.md`
  两处声明；`crates/lumilio-ui/src/map_icons.rs` 表；`lumilio-plugin-api` 的
  `MapIcon`；`lumilio-cubiomes` 的 `Structure`、`bridge.c`、金样；world explorer 的
  图层与 `i18n`。
- Out：非区域网格的地物（矿脉、洞穴、地牢、化石、sulfur spring）只入库图标，不开放
  图层；末地岛屿（cubiomes 支持但站点没有图标）；Xaero、存档底图不动。

## References

- 资源来源：<https://minecraftsearch.com/images/structures/>（站点自有 seed map 的
  标记图标集，2026-10-09 拉取）。
- `forks/cubiomes/finders.c` 的 `getStructureConfig` / `getStructurePos` /
  `isViableStructurePos`：`Treasure`、`Mineshaft`、`Desert_Well`、`Geode`、
  `End_Gateway` 都有区域配置（regionSize 1 区块）。
- `crates/lumilio-cubiomes/tests/SOURCE.md`：金样生成命令。
- ADR 0022（改编/转存代码与资源的署名方式）、`.agents/plans/world-explorer.md`。

## Tasks

- [x] T1：拉取 `images/structures/` 全部图标（52 个），WebP→PNG，替换 `map-icons/`。
- [x] T2：改 `NOTICE.md` 与两处 `ATTRIBUTIONS.md`，写清来源是 MinecraftSearch。
- [x] T3：`MapIcon` 加 `DesertWell`、`Mineshaft`、`BuriedTreasure`、`Geode`、
      `EndGateway`、`AbandonedCamp`；更新 `map_icons.rs` 表。
- [x] T4：`Structure` 加 `Treasure`、`Mineshaft`、`DesertWell`、`Geode`、
      `EndGateway`；`bridge.c` 的 `KINDS` 同步；区域上限按种类实际 regionSize 算。
- [x] T5：更新 `structures.c`，重生成 `structures.txt`，更新 cubiomes 测试。
- [x] T6：world explorer 加对应图层（`structure.buried-treasure` 等）+ 中英文案，
      废弃营地换真图标；更新插件测试。
- [ ] T7：更新 world-explorer 计划的实施记录；`just check`。

## 范围决定（2026-10-09）

站点的 feature marker（矿脉、洞穴、地牢、化石、硫磺泉、岩浆池、附魔金苹果、末地
岛屿）在 `forks/cubiomes` 里没有 finder，`StructureType` 只有标准结构；这批**只入库
图标、不接图层**，实现 finder 的后续工作记在 [`backlog.md`](backlog.md) 的「世界地图·结构」。

## Validation

- `just test-pkg lumilio-cubiomes`（金样 + 区域上限）
- `just test-pkg lumilio-plugin-world-explorer`（图层与对象）
- `just test-pkg lumilio-ui world_explorer`（图标与交互）
- `just check`

## Open questions

- 图标的取用授权：与旧一套同一立场（未授权、只作识别用），被要求时整体移除。
- Mineshaft 很密，默认不开；实机观感待维护者。
