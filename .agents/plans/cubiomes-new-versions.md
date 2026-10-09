# cubiomes 新版本世界生成：1.21.5 → 26.3

- Status: in_progress

## Goal

种子底图和结构 Overlay 支持 1.21.4 之后的版本，首个目标 26.3。新版本在 LumilioCL 自己维护的 `forks/cubiomes` 里按 W14 的五步流程补上：差异清单、改 fork、金样、对比、实机核对；做完这五步的版本才进 `lumilio-cubiomes` 的版本表，其余版本仍显示「这一版还不能查」，不降级到相邻版本。

## Scope

- In：在 `forks/cubiomes` 里补新版本（1.21.5 … 26.x）的世界生成，首个目标 26.3；`cargo xtask worldgen-diff`、`worldgen-golden`、`cubiomes-btree` 三个开发工具（不进发布包）；金样与对比测试；`forks/cubiomes/LUMILIO.md` 的逐条记录。
- Out：World Explorer 的功能阶段（P0–P7，见 [`world-explorer.md`](world-explorer.md)）；把版本表里没有的版本映射到相邻或最新的生成实现（W14）。

本计划从 [`world-explorer.md`](world-explorer.md) 的延后阶段 P8 拆出（2026-10-09），任务编号 T39–T44 保持不变。它是可选的，不阻塞 World Explorer 收尾（T45）。

## References

- [`world-explorer.md`](world-explorer.md) 的冻结决策 **W14**（cubiomes 是自维护的 fork、加版本的五步流程、入库与保留规则）；W11（cubiomes 只在 FFI crate 里）；W13（派生数据入库要有来源记录）。W14 留在 `world-explorer.md`，计划收尾时随 T45 进 ADR。
- ADR 0014（托管 Java，金样工具起服务端用）、0029（forks）、0034（读过 26.3 不混淆的客户端）。
- `forks/README.md`、`forks/cubiomes/LUMILIO.md`、`crates/lumilio-cubiomes/src/{lib.rs,tests.rs}`、`crates/lumilio-anvil/`（P4 T26，读金样世界）。
- cubiomes：<https://github.com/Cubitect/cubiomes>（`e61f905`；`biomes.h`、`biomenoise.c`、`finders.c`、`tables/btree21wd.h`、`docs/nptree_c.py`）；issue #161、#165。

## Tasks

所有功能阶段之后再做，不阻塞收尾。首个目标 26.3。1.21.5 到 26.2 之间的版本只在 T39 证明生成输入与 1.21.4 或 26.3 完全相同、且自己的金样通过时顺带支持（T43 列出结果），其余显示「这一版还不能查」。下面的已知差异来自 cubiomes issue #165；其余差异在 T39 之前一律待核实。依赖 P4 的 `lumilio-anvil`（T26）。

- [ ] T39：`cargo xtask worldgen-diff <旧版本> <新版本>`（`crates/lumilio-xtask/src/worldgen/`，新建）：按 Mojang 版本清单下载两个版本的服务端 jar 到被 git 忽略的 `target/worldgen/<版本>/`，跑数据生成器（W14 第 1 步），解出 `data/minecraft/worldgen/**` 与 `tags/worldgen/**`，输出差异摘要：主世界与下界的多噪声参数表、`noise_settings` 与密度函数、噪声参数、群系注册表、`structure_set`（salt、spacing、separation、频率、排除区）、结构的群系标签、新增结构。用得上的摘录连同 `SOURCE.md` 存进 `forks/cubiomes/data/<版本>/`。先对 1.21.4 → 1.21.5、…、26.2 → 26.3 两两跑一遍（具体的正式版列表以 Mojang 版本清单为准，待核实），结论写进 `forks/cubiomes/LUMILIO.md`。已知：26.2 新增硫磺洞穴（sulphur caves）群系（issue #165，在较低的 Y 才看得到，说明是洞穴群系、在多噪声参数表里）；它的参数、引入版本以及有没有别的变化，待核实。
- [ ] T40：`cargo xtask worldgen-golden <版本> <种子>…`：起一个本机、离线的原版服务端（固定 `level-seed`，托管 Java 由 ADR 0014 的运行时提供），对一组固定坐标 `forceload` 后用 `lumilio-anvil` 读区块的群系调色板（三个维度、几个高度），并经 RCON 在几个起点执行 `/locate structure`；结果写成 `crates/lumilio-cubiomes/tests/golden/<版本>/<种子>.json`（种子、维度、坐标、群系 id、结构种类与位置）。先对 1.21.4 跑，金样必须与未改动的上游 cubiomes 一致，证明工具本身没错。
- [ ] T41：`cargo xtask cubiomes-btree <参数表>`：从 `forks/cubiomes/data/<版本>/` 里的主世界参数表生成 `tables/btree<版本>.h`，树的构造是自己写的实现（按游戏的行为核对，不翻译游戏代码）。验收：用 1.21.4 的参数表生成的表与上游 `tables/btree21wd.h` 数值完全一致。
- [ ] T42：按 T39 的差异改 fork：`biomes.h` 新增 `MC_26_3` 等枚举项（命名跟随上游风格）与新群系 id（硫磺洞穴；若上游以后分配了 id，以上游为准，见 W14）；`tables/btree26_3.h`（若参数表变了）；`biomenoise.c` 按版本选树并处理噪声参数变化；`finders.c` 的 `getStructureConfig` 与可行群系列表（按 `structure_set` 和群系标签的差异）；下界参数表若有变化同样处理；`util.c` 的群系名称与配色（新群系先用相近群系的颜色，记进 `LUMILIO.md`）。每处改动一条 `LUMILIO.md` 记录。
- [ ] T43：若 T39 发现新结构，或已有结构的生成规则变化到现有 finder 表达不了，就为它写 finder（`finders.c`），并在 `lumilio-cubiomes` 开放；没有就在 `LUMILIO.md` 写明「26.3 无新结构」。中间版本逐个归类：生成输入与 1.21.4 相同（共用 1.21.4 的实现）、与 26.3 相同（共用 26.3 的实现）、都不同（不支持）。前两类采集各自的金样，通过后进入候选。
- [ ] T44：`lumilio-cubiomes/src/tests.rs` 用 T40 的金样逐点对比 26.3（至少三个种子 × 三个维度 × 几个高度，加上主要结构）以及 T43 的候选版本，全部一致；维护者按 W14 第 5 步实机核对 26.3 后，把 26.3 和通过的候选写进版本表（`lumilio-cubiomes/src/lib.rs`），并在 `LUMILIO.md` 记下核对日期。之后的新版本按同一流程（T39–T44）各加一次。

## Validation

- 自动：`just test-pkg lumilio-xtask worldgen`；T40 的工具在 1.21.4 上采到的金样与未改动的上游 cubiomes 一致；T41 由 1.21.4 参数表生成的表与 `btree21wd.h` 数值一致；`just test-pkg lumilio-cubiomes golden` 中 26.3 与各候选版本的金样逐点一致（三个维度、多个高度、主要结构）；`forks/cubiomes/data/` 下每个版本都有 `SOURCE.md`；仓库里没有 jar（`git ls-files '*.jar'` 为空）。
- 自动：版本表之外的版本（例如 26.2，若它未被归入候选）仍返回 `Unsupported`，界面显示「这一版还不能查」。
- **维护者实机**：在 26.3 的游戏里，用两个种子对照地图：F3 看几个采样点的群系（含硫磺洞穴等新群系，若 26.3 有），`/locate` 村庄和要塞与地图上的位置一致。核对之后才把 26.3 加进版本表。

## 风险

- 自维护 fork 的长期成本：Mojang 每个版本都可能改世界生成（新群系、参数表、噪声、结构配置），每次都要按 W14 跑一遍差异、改 C、采金样、实机核对。P8 是第一次，工作量最难估；之后每个大版本都要重复。
- 自己实现可能出错：多噪声树的构造、新群系的参数或结构规则理解错，地图就会画错而看起来很真。兜底是 W14 的金样（我们自己从真实世界测得）和维护者实机核对；金样只覆盖采样点，采样之外的错误仍可能漏掉，所以每个版本至少三个种子、三个维度、多个高度。
- 空窗期：一个新版本发布后，到我们完成五步之前，种子底图和结构 Overlay 在这个版本上显示「这一版还不能查」。玩家升级游戏后会先看到它。
- 上游 cubiomes 若恢复更新：要把上游的提交合进 fork，并逐个判断本地补丁是否被覆盖。两边对同一版本的实现不同时，以金样和实机为准（W14）；新群系的 id 若和我们自己分配的不同，要迁移种子瓦片缓存（缓存键含配色和版本，换 id 时清掉对应缓存）。

## Open questions

- 无。

## 实施记录

- 2026-10-09：从 `world-explorer.md` 的 P8 拆出成独立计划，T39–T44 原文搬入、编号不变；维护者正在推进。
