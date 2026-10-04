# 插件系统：宿主与前四个核心插件

- Status: proposed

## Goal

启动器有了插件宿主和一套扩展点，并用四个核心插件验证过：崩溃与日志分析器（纯数据）、Litematica 投影（UI 贡献）、Modrinth 内容源（外部服务与网络权限）、Discord Rich Presence（生命周期事件）。每个核心插件都能在设置里开关，关掉后它贡献的东西全部消失，启动链路不受影响。四个都跑通以后，再另开计划做 WASM 社区插件。

## 判断标准

做成核心插件，至少要满足一条：不是每个人都需要；依赖的外部服务或格式有自己的节奏；规则会持续增长，适合社区来补。

启动链路不做成插件：实例存储、安装下载、Java、账户、启动、锁。插件宿主失败时，游戏照样能启动。

## Scope

- In：
  - `lumilio-plugin-api` crate：清单、权限、扩展点 trait、数据类型，以及声明式视图树的类型。不依赖 gpui，也不依赖 `lumilio-core` 的内部实现。
  - core 里的插件宿主：注册、启用状态的持久化、权限检查、出错隔离（插件报错或 panic 只停用这个插件）、调用一律走后台任务。
  - `lumilio-ui`：把视图树渲染成 kit 组件；插件开关和权限说明放在设置页。
  - 四个核心插件，每个是一个独立的 crate，放在 `crates/plugins/<name>`，只依赖 plugin-api。
- Out：
  - WASM 运行时、社区插件的安装与分发、签名与审核。另开计划。
  - 第三方认证、备份目标这类插件。等事件和权限模型稳定之后再做。
  - 截图墙、服务器列表：人人都用，直接内置。

## 核心约束

**核心插件只能用社区插件也能拿到的 API。** 如果某个核心插件需要私有接口，就补扩展点，不开后门。

为了将来能原样搬到 WASM 上，plugin-api 的类型都要可序列化，跨边界不传借用；异步调用采用请求/响应的形式。

## References

- 崩溃规则：`3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/CrashReportAnalyzer.java`（51 条规则；我们现在只有 `diagnostics::analyze` 的 5 条）。
- 内容源接口：
  - `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/addon/RemoteAddonRepository.java`
  - 同目录 `repository/` 下的 `ModrinthRemoteAddonRepository.java`、`CurseForgeRemoteAddonRepository.java`
  - 现有的 `lumilio-core::discover` 和 ADR 0023
- Discord：`3rd-party/modrinth/packages/app-lib/src/state/discord.rs`
- 扩展模型的先例：Zed 的扩展（同样基于 GPUI；WASM + WIT；不开放任意 UI）、Obsidian 的核心插件开关。
- NBT：现有的 `lumilio-core::nbt`（有长度和嵌套深度上限）。

## Tasks

### P0 骨架

- [ ] T1：`lumilio-plugin-api`：插件清单（id、名称、版本、需要的权限）、`Plugin` trait、上下文句柄。
- [ ] T2：core 宿主：注册表、`settings.json` 里的启用状态、按清单检查权限、隔离出错的插件；附带测试。
- [ ] T3：设置页的「插件」分区：开关、权限说明、出错状态，以及每个插件的声明式设置表单（设计选择 2）。默认启用状态按设计选择 3。

### P1 崩溃与日志分析器（扩展点：纯数据）

- [ ] T4：扩展点 `Analyzer`：输入日志或崩溃报告文本、实例概况（加载器、Java、模组列表），输出 `Problem` 列表。
- [ ] T5：把 `diagnostics::analyze` 的 5 条规则迁到 `crates/plugins/crash-analyzer`，再参考 HMCL 补规则。每条规则配一个样本日志的测试。
- [ ] T6：Diagnostics → Problems 和首页「需要处理」改为汇总所有已启用的 Analyzer。关掉插件后，这些提示消失。
- [ ] T7：这个阶段落地后，把扩展点模型和 crate 边界写成决策记录（不等整个计划做完）。

### P2 Litematica 投影（扩展点：UI 贡献）

- [ ] T8：在 plugin-api 里定义视图树和 `on_action` 交互（设计选择 1），lumilio-ui 用 kit 组件来渲染。
- [ ] T9：扩展点「实例页 tab」，带出现条件（比如「实例里装了 Litematica」）和只读文件权限（只能读实例下的 `schematics/`）。
- [ ] T10：`crates/plugins/litematica`：列出投影（名称、作者、尺寸、方块数），详情页显示材料清单，可以导出成 CSV 或文本。
- [ ] T11：决定插件 UI 怎么进入生成的 IA，比如插件代码也写 `// ia[...]` 注释，或者按插件单独生成一页。

### P3 Modrinth 内容源（扩展点：外部服务）

- [ ] T12：把 core 里写死 Modrinth 的 16 个文件梳理一遍，抽出 `ContentSource`：搜索、项目、版本、文件、依赖、分类与筛选能力。筛选模型和 URL 拼接分开，用 CurseForge 纸面验证（设计选择 4）。
- [ ] T13：权限里加入「网络：指定域名」，宿主统一走 `fetch`，插件不能自己开连接。
- [ ] T14：Modrinth 实现迁到 `crates/plugins/modrinth`。整合包安装、更新检查、依赖解析都改为通过 `ContentSource` 调用，前后行为不变，现有测试全部通过。
- [ ] T15：关掉 Modrinth 以后，发现页显示「没有可用的内容源」，已安装的内容照常可用。

### P4 Discord Rich Presence（扩展点：生命周期事件）

- [ ] T16：事件订阅：游戏启动、进入世界、退出、启动失败。事件只能观察，不能阻断启动。
- [ ] T17：权限里加入「本地能力：Discord IPC」，由宿主提供；这类能力只开放给核心插件。
- [ ] T18：`crates/plugins/discord`，参考 Modrinth 的 `discord.rs`。Discord 不在运行时安静地跳过，不报错。

## Validation

- 每个阶段跑完四项检查。plugin-api 和宿主的逻辑都在 core 侧，有单元测试。
- 每个插件都要证明两点：启用时它的贡献出现，停用后全部消失；插件 panic 或报错时，只有它自己被停用，启动和其他插件不受影响。
- 依赖方向检查：插件 crate 只依赖 `lumilio-plugin-api`，不依赖 `lumilio-core` 或 `lumilio-ui`。用测试读 `cargo metadata` 来保证，不靠人记。
- 维护者肉眼验收：设置里的插件分区、投影 tab、发现页没有内容源时的样子。

## 已定的设计选择（暂定，落地时写进决策记录）

1. **视图树只描述内容，不描述布局。** 插件说「这是一个列表，每行有标题、副标题、数值和操作」，宿主决定怎么画、用哪个 kit 组件、间距和动效是多少。所以没有 div、flex、颜色、尺寸这类原语，插件的界面自动符合 design-language，也不会长成第二套 UI 框架。
   - 第一版组件只取 P2 用得到的：区块（标题 + 内容）、列表（行可以点开详情）、详情（头部 + 键值事实 + 区块）、表格、空状态、文本（正文 / 次要 / 等宽三级）、标签、图片（字节由插件给，缩放由宿主做）、按键（操作，可以标 `destructive`，由宿主负责确认弹窗）。
   - 交互采用 Elm 式：插件收到 `on_action(id)`，返回新的视图树，由宿主比对后重画。插件不持有 UI 状态，这样搬到 WASM 上也是同一套模型。
   - 增长规则：加一种组件，需要两个插件都用得上，或者一个核心插件用得上并同时写进 design-language。3D 预览第一版用插件在 CPU 上渲染出的图片代替，不开放绘图原语。
2. **插件设置用声明式表单，和视图树分开。** 设置有统一的语义：持久化、默认值、校验、恢复默认。这些应该由宿主负责，存在 `settings.json` 里各插件自己的命名空间下，在「设置 → 插件 → 某个插件」里按 design-language §10 的设置行来画。
   - 字段类型：开关、单选（分段键或下拉）、文本、带范围的数字。路径选择属于权限问题，第一版不给。
   - 插件通过上下文读取设置值，值变化时会收到通知。不提供「自己画设置页」的出口。
3. **默认是否启用，看它会不会主动出现、会不会把数据往外送。**
   - 平时看不见、只在相关时出现，且不往外发数据的：默认启用。崩溃分析器（没发现问题就看不见）、Modrinth（没有它发现页是空的）、Litematica 都属于这类。
   - 往第三方发数据或影响启动的：默认关闭，由用户主动打开。Discord 属于这类，它会告诉别人你在玩什么。
   - 「检测到相关模组再出现」靠扩展点自带的出现条件来实现（T9），不弹窗提示用户去启用：提示属于打扰，出现条件不打扰。没有首次运行的插件向导。
4. **P3 在 ADR 0023 的发现页落地并提交后再开工；P0–P2 和它无关，可以先做。**
   - P3 的第一步是读落地后的 `discover/query.rs`。它现在把筛选模型（`Pick` / `Stance`）和 Modrinth 的 URL 拼接写在一起，需要拆开：筛选模型留在 core，作为 `ContentSource` 的输入；拼 URL 归 Modrinth 插件。
   - 内容源要声明自己支持哪些筛选（包含/排除、环境、许可证……），发现页按声明显示筛选项。
   - 接口定稿前，拿 HMCL 的 `CurseForgeRemoteAddonRepository.java` 纸面走一遍，证明第二个内容源也能实现它，避免做出一个 Modrinth 形状的接口。
