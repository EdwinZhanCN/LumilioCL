# UI 信息架构（IA）

`ARCH.md` 是产品的骨架；本目录把骨架上的每个节点展开到**用户可达的每一条路径**：页面有哪些信息层级、每一层放什么、用什么组件、点了之后去哪、需要什么底层能力。实现任何 UI 前先读对应页面的 IA；IA 里没有的东西不做，做了的东西 IA 里必须有。

视觉、动效、文案规则在 [design-language.md](../design-language.md)；业务契约在 [workflows](../workflows/README.md)；本目录只回答“页面上有什么、在哪一层、用什么组件”。

## 来源与取舍

- **功能完整性**对照 HMCL（[hmcl-reference](../workflows/hmcl-reference.md) 的 H 编号）：HMCL 能做到的玩家日常功能，我们默认都要有，除非标为本版不做。
- **界面结构**参考 Modrinth App 的现代桌面布局，取其清晰的层级（页头 · 标签 · 工具栏 · 列表），不照搬其平台功能（好友、同步、托管、皮肤商店、广告位）。
- 冲突时以 `design-language.md` 的规则为准（§7 页面结构、§10 控件与编辑、§11 消息）。

## 层级词汇

每个页面都用同一套层级描述（design-language §7）：

| 层 | 名称 | 内容 | 常用组件 |
|---|---|---|---|
| L0 | 导航 | 全局导航栏：历史 + 位置名 · 地标 · 当前游戏/账户 | 见 [navigation](navigation.md) |
| L1 | 页头 | 标题、说明行；详情页带封面/图标、标签、统计 | `kit::header` |
| L2 | 页面操作 | 次要 · 主要 · 更多（⋯） | `kit::PageActions`、`DropdownMenu` |
| L3 | 工具栏 | 视图标签（ARCH 层级）+ 页内搜索 | `kit::tabs`（接口标签条）、`Input` |
| L4 | 细化 | 子分类分段、排序、筛选、分页、批量选择栏 | `kit::segments`（带 LED 的分段键）、`Select`、筛选侧栏 |
| L5 | 内容 | 卡片网格 / 列表块 / 设置组；加载、空、出错在这一层原位替换 | `kit::list`（发丝线表格）、`kit::panel_list`（集合面板）、`kit::value_row`、`kit::faceplate`（游戏卡片） |
| L6 | 浮层 | 弹窗（编辑、创建、确认）、弹出列表、toast | `Dialog`、`Popover`、`Notification` |

## 状态

**已实现的用户路径不再手写。** 实现处的 `// ia[page]: …` 注释是唯一来源，`cargo run -p lumilio-docgen -- ia` 生成 [paths/](paths/)；表里有一行就等于做了，删掉功能时注释跟着没了，`cargo test` 在生成物过期时失败（ADR 0019）。

手写的页面文档只写页面定位、层级草图、版面描述，以及**没做的**路径，用下面的标记：

| 标记 | 含义 |
|---|---|
| 🟨 | core 已有能力，UI 待做 |
| 🟥 | core 也缺，需先做领域能力 |
| ⏸ | 本版不做（写明原因或需要的 ADR） |

## 页面索引

| 页面 | 文件 | ARCH 节点 |
|---|---|---|
| 全局导航 | [navigation.md](navigation.md) | APP |
| 共享模式与组件规范 | [patterns.md](patterns.md) | — |
| 首页 | [home.md](home.md) | Home |
| 游戏库（含新建游戏） | [library.md](library.md) | Library |
| 发现（含项目详情） | [discover.md](discover.md) | Discover |
| 动态 | [activity.md](activity.md) | Activity |
| 账户 | [accounts.md](accounts.md) | Accounts |
| 设置 | [settings.md](settings.md) | Settings |
| 游戏（实例）页 | [instance/README.md](instance/README.md) | Instance |
| · 概览 | [instance/overview.md](instance/overview.md) | Instance.Overview |
| · 内容 | [instance/content.md](instance/content.md) | Instance.Content |
| · 世界 | [instance/worlds.md](instance/worlds.md) | Instance.Worlds |
| · 历史 | [instance/history.md](instance/history.md) | Instance.History |
| · 诊断 | [instance/diagnostics.md](instance/diagnostics.md) | Instance.Diagnostics |
| · 设置 | [instance/settings.md](instance/settings.md) | Instance.Settings |

## 每份页面文档的结构

1. **定位**：这个页面替用户解决什么，从哪些入口进来。
2. **信息层级**：按 L1–L6 列出每一层的元素、组件和规则。
3. **用户路径**：已实现的由代码注释生成（见上），页面文档里只留没做的。
4. **状态**：加载、空、出错、忙碌各自长什么样。
5. **范围外 / 待决策**。

## 落地状态

本目录内已没有 🟨 / 🟥 项；各页面的 ✅ 以代码为准。已完成的计划见 `.agents/plans/history.md`。

仍然没做：离线皮肤（ADR 0018 提议推迟）、第三方认证、CurseForge 整合包、启动器自动更新、整合包更新。
待维护者实机验收：Microsoft 登录（计划 0031）、更换版本（计划 0030）、各页面的视觉验收。
