# 待办：没做、还没排期的

想做但还没开计划的事，一行一件，写明前提。开工时从这里删掉，另开一个计划；做完了就不该还在这里。已经做了什么，看生成的 `docs/ia/paths/`。

## 产品

| 范围 | 没做的 | 前提 / 原因 |
|---|---|---|
| 账户 | `authlib-injector:` 链接拖入 | ADR 0024 |
| 账户·外观 | 皮肤库下的「默认皮肤」分组（Modrinth 有）；正版账户穿默认皮肤要上传那张图 | 从已安装客户端 jar 列出全部默认皮肤，不进仓库（ADR 0024、0039） |
| 设置 | 启动器自动更新 | 需要 ADR |
| 发现 | CurseForge 等其他来源；下载世界 | 需要来源 ADR；CurseForge 要 API 密钥 |
| 发现 | 数据包页签（Modrinth 装进 `<游戏>/datapacks`，原版不读，要先有世界级数据包管理）；多人服务器页签；「依赖于 / 包含内容」筛选（要项目选择器）；解锁筛选后安装不适配版本（核心有意拒绝，ADR 0023） | 见 ADR 0023 |
| 游戏库 | 从 CurseForge 导入；自定义实例图标；外部游戏目录登记 | CurseForge 要 API 密钥；图标优先用像素封面；外部数据根不在 ADR 0007 首轮 |
| 游戏页·内容 | 固定版本；拖入文件；「全部更新」里逐项取消勾选；识别结果缓存；自动补依赖 | 固定版本要在实例数据里记录 |
| 游戏页·世界 | 世界属性/玩家数据编辑、数据包管理、在线下载世界；服务器列表的 SRV 记录解析 | SRV 需要 DNS 依赖 |
| 游戏页·诊断 | 文件编辑、NBT 编辑、线程 dump | — |
| 游戏页·投影 | 26.2+ 方块模型告示牌上的文字；不同颜色的染色玻璃不应互相剔除 | 在 `forks/` 修复并加回归测试；ADR 0034 |
| 游戏页·投影观察 | Wayland 原生相对指针与鼠标锁定 | 接入 GPUI 的 relative-pointer / pointer-constraints 协议；当前明确提示不支持捕获，Orbital 可用；ADR 0036 |
| 整合包 | 整合包更新（识别包管理文件）；整合包内容单独更新时的提示 | — |
| 恢复 | 恢复模式界面、数据迁移、未完成安装/更新的恢复、进程失联接管、跨卷删除 | ADR 0007 P2/P3 |
| 世界地图·结构 | 站点 Seed Map 上的 feature marker：矿脉（Iron/Copper Vein）、Cheese Cave、Ravine、Lava Pool、Dungeon×3、Fossil / Nether Fossil、Sulfur Spring、Enchanted Golden Apple，以及末地岛屿 | `forks/cubiomes` 的 `StructureType` 只有标准结构，没有这些 finder，要自行实现（站点用 `cubiomesType` 201–223 是他们自己的 fork）。图标已全部入库 `crates/lumilio-ui/assets/map-icons/`（2026-10-09），只差 finder 与图层。Maps 的结构图标来源见同目录 `NOTICE.md` |

## 只有维护者能做

- 世界/服务器封面与账户头像：实机查看有图、无图、在线与离线状态下的外观，并确认 40px 世界/服务器前导图与行高是否合适。
- 世界地图结构图层：实机查看 Mineshaft 标记密度；默认仍关闭该图层。

- Litematica 原生预览（ADR 0028）：Windows/Linux 和更老游戏版本的兼容检查；本机原生预览与绘制规则已由维护者验收。
- ADR 0007 仍是 `proposed`，ADR 0018 推迟中。
