# Xaero 路径点管理插件

- Status: proposed

## Goal

安装了 Xaero's Minimap 的实例上，多一个核心插件页。人可以按世界和维度查看路径点集合，修改名称、缩写、坐标、16 色、启用状态和传送朝向，新建和删除普通路径点，并复制游戏内能识别的分享串。游戏正在运行时只读，退出后再写回 Xaero 自己的文件。

## Scope

- In：读取和写回一个实例里 Xaero's Minimap 的路径点文件；集合切换；分享串复制；写权限只覆盖 `xaero/minimap`。
- Out：小地图渲染、游戏内传送、VoxelMap / JourneyMap 的原生格式、在游戏运行中抢写、死亡点的批量清空。

## 调研

Xaero's Minimap 把路径点放在实例的 `xaero/minimap/` 下，按世界再按维度分目录。单人用世界目录名，多人用 `Multiplayer_<地址>` 这类目录。维度目录常见形式是 `dim%0`（主世界）、`dim%-1`（下界）、`dim%1`（末地），新版本也可能使用带命名空间的目录名。集合文件名如 `mw$default.txt`。

社区对分享串的记录（<https://gist.github.com/macimas/937a392be075b1bce7a2ae69ea933ef5>）是：

`xaero-waypoint:name:marker:x:y:z:color:use_yaw:yaw:dimension`

`color` 是 0–15 的下标，不是任意 RGB。XaeroPlus 的讨论确认颜色表只有这 16 个预设。磁盘上的集合文件比分享串多几个字段：禁用、类型、所属集合、传送时是否改朝向、可见性。文件头有 `sets:` 和 `dim:`。这个 gist 不是 Xaero 的规范，实现时要以选定版本的写入代码为准，并在测试里钉住一份真实文件。

JourneyMap 只在发现当前世界已有 Xaero 数据时提供导入（<https://teamjm.github.io/journeymap-docs/6.0.x/client/waypoints>）。它按单人世界、服务器地址或 Realm 匹配目录。本插件做的是直接编辑，不是从别的地图导入。

现有插件视图树适合列表和表单，不需要新的绘图原语。现有权限只有游戏目录的只读范围（ADR 0031 D3）。写回必须新增一条窄权限：只能写 `xaero/minimap` 下的路径点文件，不能写整个实例。

Xaero 在退出世界时会把自己的内存写回磁盘。实例进程还在时，启动器的写入会被覆盖。所以运行中只显示，并说明退出游戏后再改。

临时路径点和死亡点要能看见。默认的删除和批量操作只作用于普通、启用、非临时的点；死亡点需要单独确认。

## References

- ADR 0031，尤其是 D3 权限和 D5 视图树
- Xaero 路径点分享串记录：<https://gist.github.com/macimas/937a392be075b1bce7a2ae69ea933ef5>
- 颜色下标：<https://github.com/rfresh2/XaeroPlus/issues/301>
- JourneyMap 对目录匹配的说明：<https://teamjm.github.io/journeymap-docs/6.0.x/client/waypoints>

## Tasks

- [ ] T1：在插件 crate 里解析并写回集合文件。未知字段原样保留。用一份从当前 Xaero 版本导出的主世界、下界、末地样本做往返测试，样本放在测试数据里并注明版本。
- [ ] T2：宿主增加 `xaero/minimap` 的写权限。插件声明这条权限；设置页用一句话说明它会改该实例的路径点文件。读失败、目录不存在、文件损坏都变成页面状态，不进入插件 `Failed`。
- [ ] T3：实例页列出世界、维度和集合。行内可改名称、缩写、坐标、颜色、启用、朝向。新建普通点，删除前确认。复制 `xaero-waypoint:` 分享串。
- [ ] T4：实例正在运行时禁用写入，保留浏览和复制。

## Validation

- 样本文件读出后再写回，除有意修改的字段外逐字节一致。
- 损坏文件显示错误，不覆盖原文件。
- 写权限测试证明插件碰不到 `xaero/minimap` 以外的路径。
- 运行中的实例不会发出写文件效果。
- 实机：改一个点，重新进入世界后 Xaero 列表里看得到。

## Open questions

- Xaero's World Map 新版本与小地图共用这套文件。若选定版本已经分叉成另一套目录，只支持小地图那一套，并在页面上说明。
- 名称里的冒号和换行会破坏分列格式。保存时拒绝或转义，以实现时读到的写入器为准，不能自创一种 Xaero 不认识的转义。
