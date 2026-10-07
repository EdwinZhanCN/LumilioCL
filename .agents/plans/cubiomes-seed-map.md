# cubiomes 种子地图插件

- Status: proposed

## Goal

实例页多一个核心插件。它用 cubiomes 按该实例的游戏版本和世界种子，画出主世界、下界、末地的生物群系，并叠加上结构、史莱姆区块和出生点。人可以平移、缩放、开关图层，并点一个结构复制坐标。计算在后台进行，取消和切种子会丢掉过期的图块。

## Scope

- In：Java 版、cubiomes 已支持的版本；当前实例的种子和版本；生物群系图块、结构标记、史莱姆区块、出生点；宿主绘制的地图，插件只提供图块和标记。
- Out：基岩版、在地图上创建路径点、替代 Amidst / Chunkbase 的全部种子筛选条件、把 cubiomes 链进 core、用插件视图树直接画 GPU。

## 调研

Chunkbase 和 Cubiomes Viewer 都用 Cubitect 的 cubiomes 复现 Java 版生物群系与结构位置。库的入口是 `generator.h`、`biomes.h` 和 `finders.h`：`Range` 的水平缩放只支持 1、4、16、64、256，垂直方向在 `scale == 1` 时是 1:1，否则是 1:4（<https://github.com/Cubitect/cubiomes>）。Cubiomes Viewer 覆盖到 Java 1.21，许可证是 GPL-3.0；作者写明 1.18 之后沙漠神殿、丛林神殿和林地府邸会因为地形不合适而生成失败，库只能按生物群系和气候噪声估计，结果会有偏差；1.18 之前的出生点有时不准，因为它取决于草方块（<https://github.com/Cubitect/cubiomes-viewer>）。

本仓库是 AGPL-3.0。GPLv3 第 13 条允许与 AGPL 组合，组合结果继续按 AGPL 发布。cubiomes 放在这个插件 crate 里，保留 Cubitect 的版权与 GPL 声明，写进 `ATTRIBUTIONS.md`。不要为了省事把 C 库链进 `lumilio-core`。

ADR 0031 的视图树只描述列表和文字，插件不能拿绘图原语。一张可平移的地图如果交给插件自己画，就破坏这条边界。宿主增加一种地图节点：插件给出已算好的图块和标记，宿主负责平移、缩放和命中测试。图块请求带种子、版本、维度、缩放和范围；迟到的结果因请求 id 不匹配而被丢掉，和投影弹窗的做法一样。

现有插件调用是 `spawn_blocking` 加数秒超时。一幅地图会连续要很多图块，5 秒超时和失败后停用到重启都不合适。图块任务可取消，失败只让这块图显示重试，不把插件打成 `Failed`。

种子来自该实例的 `level.dat`。没有世界时，页面要求人粘贴种子，并写明版本，而不是猜一个。

## References

- cubiomes：<https://github.com/Cubitect/cubiomes>
- Cubiomes Viewer 的版本范围和已知误差：<https://github.com/Cubitect/cubiomes-viewer>
- ADR 0031 D1、D2、D5
- 仓库根目录 `LICENSE`（AGPL-3.0）

## Tasks

- [ ] T1：插件 crate 用构建脚本编译 cubiomes。包装「给定版本、种子、维度、范围，返回生物群系 id」和「返回结构位置」。不支持的版本给出明确错误。用 cubiomes 自带测试里的已知种子做对照，而不是只看图。
- [ ] T2：宿主视图协议增加地图节点和取消令牌。插件在后台填图块；新的视口取消旧任务。超时或崩溃只影响这一次查找。
- [ ] T3：实例页读到种子和数据版本后显示地图。图层至少包括生物群系、史莱姆区块、出生点，以及该版本 finder 能给出的村庄、神殿、要塞、海底神殿、下界要塞、堡垒、末地城。点标记复制 `x z`。
- [ ] T4：在页面上写明 1.18+ 部分结构是估计位置。没有 `level.dat` 时走手动种子。

## Validation

- 至少三个版本（1.16、1.18、1.21 或库实际支持的最新版）的固定种子，结构坐标与 cubiomes 命令行输出一致。
- 快速拖动地图时，旧图块不能画进新种子或新维度。
- 插件崩溃测试证明宿主还在，地图显示失败状态。
- 构建产物的归属声明包含 cubiomes 的 GPL 通知。
- 实机：用一个已知种子对照 Chunkbase 或 Cubiomes Viewer 的同一处村庄和要塞。

## Open questions

- 1.21 之后的版本要等 cubiomes 上游。页面显示「这一版还不能查」，不降级到相邻版本假装准确。
- 生物群系配色第一轮使用 cubiomes 自带的 Amidst 风格色表，并在归属里注明。不单独做主题化配色。
