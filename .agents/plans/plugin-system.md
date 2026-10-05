# 插件系统：宿主与前四个核心插件

- Status: in_progress

## Goal

启动器有了插件宿主和一套扩展点，并用四个核心插件验证过：

- 崩溃与日志分析器（纯数据）
- Litematica 投影（UI 贡献）
- Modrinth 内容源（外部服务与网络权限）
- Discord Rich Presence（生命周期事件）

每个核心插件都能在「设置 → 插件」里开关，关掉后它贡献的东西全部消失，启动链路不受影响。四个都跑通以后，再另开计划做 WASM 社区插件。

## 接手须知（每个会话开工前先读这一节）

1. **按阶段顺序做：P0 → P1 → P2 → P3 → P4。** 前一阶段的「验收」全部打勾，并且已经提交，才能开下一阶段。一个阶段可以分几次提交，每次提交都必须通过 pre-commit 钩子里的四项检查。
2. **「冻结的决定」不能自己改。** 如果实施中发现某条行不通，先停下，在本文件末尾的「实施记录」里写清楚哪里不通、证据是什么、建议怎么改，然后问维护者。不要绕过去，也不要悄悄换一种做法。
3. **「留给实施者的细节」可以自己定**，定了以后写进「实施记录」。
4. **出现下面任何一种情况，说明走偏了，先停下：**
   - 插件 crate 依赖了 `lumilio-core`、`lumilio-ui`、`lumilio-app` 或 gpui。
   - 为某个插件在 `lumilio-ui` 里写专门的渲染代码。插件的界面只能来自视图树。
   - 为插件里的某一条规则在 core 里加一个 enum 变体，比如给每种崩溃原因加一个 `ProblemKind`。
   - 插件自己开网络连接、自己读写任意路径，或者在 UI 线程上执行插件代码。
   - 开始做 WASM、插件市场、从磁盘加载插件。这些不在本计划里。
5. 每个阶段做完，在下面的任务上打勾，并在「实施记录」里追加一段：做了什么、踩了什么坑、哪些需要维护者肉眼看。

## 判断标准（什么做成插件）

做成核心插件，至少要满足一条：

- 不是每个人都需要。
- 依赖的外部服务或格式有自己的节奏。
- 规则会持续增长，适合社区来补。

启动链路不做成插件：实例存储、安装下载、Java、账户、启动、锁。截图墙、服务器列表、第三方认证（ADR 0024）已经内置，不改成插件。

## 冻结的决定

### D1 crate 结构与依赖方向

```text
lumilio-plugin-api   （新）只依赖 serde、serde_json；定义清单、权限、扩展点 trait、数据类型、视图树
lumilio-nbt          （新）从 lumilio-core/src/nbt.rs 原样拆出；只依赖 flate2
lumilio-core         依赖 plugin-api 和 lumilio-nbt；内含插件宿主 `plugins` 模块
lumilio-ui           依赖 plugin-api（渲染视图树、设置表单）
lumilio-app          依赖每个插件 crate，启动时把它们交给宿主
lumilio-plugin-crash-analyzer / -litematica / -modrinth / -discord
                     只能依赖 plugin-api、lumilio-nbt 和通用库（serde、serde_json、regex、url……）
```

- 插件 crate 平铺放在 `crates/lumilio-plugin-<name>/`，**不要**放进 `crates/plugins/` 子目录。原因：docgen（IA）和 `crates/lumilio-docgen/tests/attribution.rs` 只扫描 `crates/*/src`。
- P0 加一个测试 `crates/lumilio-docgen/tests/plugin_boundaries.rs`：读每个 `crates/lumilio-plugin-*/Cargo.toml` 的依赖，出现 `lumilio-core`、`lumilio-ui`、`lumilio-app` 或任何 `gpui*` 就失败；插件 crate 没有依赖 `lumilio-plugin-api` 也失败。

### D2 插件 API 是同步的，由宿主负责异步

- 插件的所有方法都是普通的同步函数。宿主在 `tokio::task::spawn_blocking` 里调用插件，外面套超时（默认 5 秒，内容源的搜索 20 秒），再用 `std::panic::catch_unwind(AssertUnwindSafe(..))` 兜住 panic。
- 插件需要网络或文件时，调用宿主通过上下文提供的函数（`ctx.fetch`、`ctx.read_file`）。这些函数在宿主里检查权限，再用 `Handle::block_on` 执行。将来搬到 WASM 上，这些就是宿主函数，语义不变。
- 插件出错、超时或 panic：这一次调用返回空结果，插件进入 `Failed { message }` 状态，在本次运行期间停用，设置页显示原因。这个状态不写进 `settings.json`，重启后自动恢复。

### D3 清单与权限

```rust
pub struct Manifest {
    pub id: String,              // "lumilio.crash-analyzer"：小写、点分，全局唯一
    pub name: String,            // 设置页显示的中文名
    pub description: String,     // 一句话
    pub version: String,
    pub api: u32,                // 等于 plugin_api::API_VERSION，否则宿主拒绝加载
    pub default_enabled: bool,
    pub permissions: Vec<Permission>,
    pub settings: Vec<SettingField>, // 见 D6
}
pub enum Permission {
    ReadGameFiles { under: String },    // 只读实例游戏目录下的这个子目录，比如 "schematics"
    Network { hosts: Vec<String> },     // 只能访问这些主机（精确匹配）
    LaunchEvents,                       // 订阅生命周期事件
    Native(NativeCapability),           // 只给核心插件；目前只有 DiscordIpc
}
```

宿主在每次调用上下文函数时检查权限，越权就返回错误，不 panic。设置页逐条用人话显示权限，比如「读取游戏的 schematics 文件夹」「访问 api.modrinth.com」。

### D4 是否默认启用

- 平时看不见、只在相关时出现、不往外发数据的，默认启用：分析器、Litematica、Modrinth。
- 往第三方发数据的，默认关闭：Discord。
- 不做「检测到模组后弹窗提示启用」。由出现条件决定 tab 是否显示（见 D5）。
- 启用状态存在 `LauncherSettings.plugins: BTreeMap<String, PluginState>`，其中 `PluginState { enabled: Option<bool>, values: BTreeMap<String, SettingValue> }`。`enabled` 为 `None` 时取清单里的默认值。字段加 `#[serde(default)]`，**不升** settings 的 schema 版本，因为这是纯新增的字段。

### D5 视图树只描述内容（P2 实现）

- 组件只描述「这里是什么」，不描述「怎么画」：没有 div、flex、颜色、尺寸、间距。第一版只有这些：
  - `Section { title, children }`
  - `List { items }`：每行 `ListItem { id, title, subtitle, value, image, tags, open: Option<ActionId> }`
  - `Detail { title, subtitle, image, facts: Vec<(String, String)>, children }`
  - `Table { columns, rows }`
  - `Empty { title, message }`
  - `Text { text, tone: Body | Secondary | Mono }`
  - `Tags(Vec<String>)`
  - `Image(ImageData)`：RGBA 像素和宽高，由宿主缩放
  - `Key { id: ActionId, label, kind: Primary | Ghost, destructive: bool }`：`destructive` 的确认弹窗由宿主出
- 交互采用 Elm 式：插件实现 `fn view(&self, ctx, state: &TabState) -> View` 和 `fn update(&self, ctx, state: TabState, action: ActionId) -> (TabState, Vec<Effect>)`。`TabState` 是一个 `serde_json::Value`，由宿主按「实例 + 插件」保存。插件自己不保存 UI 状态。
- 插件不能直接动磁盘或系统界面。需要这类操作时，在 `update` 里返回 `Effect`，由宿主替它执行。第一版只有三种：
  - `RevealGameFile { path }`：在文件夹中显示。`path` 必须落在插件 `ReadGameFiles` 权限允许的目录下。
  - `SaveAs { suggested_name, bytes }`：宿主弹出保存对话框，用户选了位置才写入。用户的选择本身就是同意，所以不需要额外权限。
  - `Toast(text)`：显示一条提示。
- 加一种组件，需要有两个插件都用得上，或者一个核心插件用得上并同时写进 `docs/design-language.md`。不开放绘图原语。

### D6 插件设置用声明式表单

- `SettingField { key, label, help, kind }`，`kind` 只有四种：`Toggle { default }`、`Choice { options, default }`、`Text { default }`、`Number { min, max, default }`。
- 宿主按 design-language §10 的设置行来画，统一负责持久化、校验和恢复默认。插件通过 `ctx.setting(key)` 读值。设置变了，下一次调用时插件就会读到新值，不另外推送通知。

### D7 IA

- 插件贡献的 UI，`// ia[...]` 注释写在插件 crate 里构建对应视图的地方。页面 key 用 `plugin.<短名>`，比如 `plugin.litematica`，并在 `crates/lumilio-docgen/src/lib.rs` 的 `PAGES` 里加一行。
- 插件开关、权限、设置表单属于设置页，页面 key 是 `settings`，注释写在 `lumilio-ui`。

## 留给实施者的细节（定了写进实施记录）

- 宿主的具体类型名，比如 `PluginHost`、`PluginStatus`。
- 超时的具体数值（在 D2 的量级内）。
- 设置页插件分区的版式（遵守 design-language）。
- 实例页的插件 tab 排在哪里：要求是内置 tab 的序号和 `LUMILIO_PAGE` 的行为不变。建议排在「截图」之后、「历史」之前，并把 tab 序号改为 `enum`。
- 每个插件的显示名和描述文案（遵守 design-language 的文案规则）。

## 阶段与任务

### P0 骨架

入口条件：无。

- [x] T1 新建 `crates/lumilio-plugin-api`：`API_VERSION = 1`；`Manifest`、`Permission`、`SettingField`、`SettingValue`；`Plugin` trait。每个扩展点是一个返回 `Option<&dyn Trait>` 的方法，默认返回 `None`。另有上下文 trait `HostContext`，提供 `setting`、`read_file`、`fetch`（P0 先只实现 `setting`，其余返回「未授权」）。
- [x] T2 从 `lumilio-core/src/nbt.rs` 原样拆出 `crates/lumilio-nbt`，core 改为依赖它，行为不变，原有测试跟着搬过去。
- [x] T3 core 新增 `plugins` 模块：注册、按 `API_VERSION` 拒绝不兼容的插件、按 `LauncherSettings.plugins` 计算启用状态、按 D2 隔离调用、`Failed` 状态。`LauncherService::open` 增加插件列表参数，app 在 `backend.rs` 里传入；测试里传空列表。
- [x] T4 设置持久化：`LauncherSettings.plugins`（D4）；加一个服务方法，用来设置启用状态和设置值。
- [x] T5 设置页新增「插件」tab：开关、描述、权限（人话）、失败原因、声明式设置表单（D6），并为它写 `// ia[settings]` 注释。
- [x] T6 `crates/lumilio-docgen/tests/plugin_boundaries.rs`（见 D1），并证明它能失败：临时让一个插件依赖 core，看到测试变红后再撤掉。
- [x] T7 加一个只在测试里用的假插件（放在 core 的测试里，不进产品），覆盖：启用和停用、panic 被兜住并标为 `Failed`、超时、越权调用被拒绝、`API_VERSION` 不匹配时被拒绝。

验收：

- [x] 设置页能看到一个空的插件列表（P0 还没有真插件），四项检查通过。
- [x] 假插件的测试覆盖了 T7 列出的每一种情况。

### P1 崩溃与日志分析器（扩展点：纯数据）

入口条件：P0 已提交。

- [x] T8 在 plugin-api 里定义扩展点 `Analyzer`：
  - 输入 `AnalysisInput { text, source: LatestLog | CrashReport, game: GameFacts }`，其中 `GameFacts` 包括游戏版本、加载器和加载器版本、Java 主版本、`mods: Vec<ModFact { id, version, file }>`。
  - 输出 `Vec<Finding { rule, severity, title, advice, evidence: Option<String> }>`。`title` 和 `advice` 直接是给人看的中文。
- [x] T9 新建 `crates/lumilio-plugin-crash-analyzer`：把 `diagnostics::analyze` 的 5 条规则原样迁过来，测试一起迁。之后参考 `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/CrashReportAnalyzer.java` 补规则，按常见程度排序，第一批至少补 15 条。每条规则配一段样本日志测试，正反例都要有。从 HMCL 改编的规则按 ADR 0011 注明来源（attribution 测试会检查）。
- [x] T10 core：删掉 `CrashHint` 和 `diagnostics::analyze`，`crash_report` 改为返回宿主汇总的 `Vec<PluginFinding { plugin, finding }>`。UI 删掉 `hint_text`，原样显示 `title` 和 `advice`。
- [x] T11 问题列表：最近一次游戏崩溃或启动失败时，`problems()` 对最新的崩溃报告（没有报告就用 `latest.log`）跑一遍分析器，每条结果作为 `ProblemKind::Finding(PluginFinding)`。首页「需要处理」显示其中最严重的一条。
- [x] T12 停用分析器插件后，崩溃报告页和问题列表里不再出现分析结果，但原有的内置问题（缺 Java、Mod 重复等）照常显示。

验收：

- [x] 原来 5 条规则的行为不变，现有测试迁移后全部通过。
- [x] 新规则每条都有测试。
- [x] 停用和启用的效果都有测试覆盖。
- [x] 维护者要用肉眼看：一个真实的崩溃报告在诊断页上的显示效果。

### P2 Litematica 投影（扩展点：UI 贡献）

入口条件：P1 已提交。

- [x] T13 在 plugin-api 里定义 `View`、`ActionId`、`TabState`、`Effect`（D5），以及扩展点 `InstanceTab`：
  - `title()`
  - `appears(&GameFacts, ctx) -> bool`：出现条件
  - `view(...)`
  - `update(...)`
- [x] T14 core：宿主保存每个「实例 + 插件」的 `TabState`，并执行插件返回的 `Effect`（`RevealGameFile` 先校验路径）；新增服务方法 `plugin_tabs(instance)`、`plugin_view(instance, plugin)`、`plugin_action(instance, plugin, action)`。实现 `ctx.read_file`：只允许读 `ReadGameFiles.under` 下面的文件，用 `Path::components` 拒绝 `..`、绝对路径和符号链接；单个文件的大小有上限。
- [x] T15 lumilio-ui：只写一个通用的视图树渲染器，`View` 的每个组件对应一个 kit 组件；`destructive` 的键由宿主出确认弹窗。实例页接入插件 tab（见「留给实施者的细节」），沿用 UI 发意图 → app 调服务 → 数据回到 UI 的模式，可参考截图墙的提交 `f15b81e` 有哪些接入点。
- [x] T16 新建 `crates/lumilio-plugin-litematica`：
  - 出现条件：实例的 mods 里有 Litematica（按 mod id 判断），或者游戏目录下存在 `schematics/`。
  - 列表：读取 `schematics/**/*.litematic`，用 `lumilio-nbt` 解析 `Metadata`（Name、Author、Description、EnclosingSize、TotalBlocks、TimeModified），有 `PreviewImageData` 就显示缩略图。
  - 详情：键值事实，加上材料清单表格（方块 id、数量），按数量降序。材料清单要解码各个 Region 的 `BlockStatePalette` 和 `BlockStates`（位宽 = max(2, ⌈log₂ 调色板长度⌉)，值可以跨越两个 long）。照格式规范自己实现，不要复制 Litematica 的源码（它是 LGPL，没有放进 3rd-party）。
  - 操作：「在文件夹中显示」（`Effect::RevealGameFile`）和「导出材料清单（CSV）」（`Effect::SaveAs`）。插件不能直接写盘（D5）。
  - 测试用 `lumilio-nbt` 的写入函数在测试里构造 `.litematic`，不往仓库里提交二进制样本。
- [x] T17 IA：按 D7 写 `// ia[plugin.litematica]` 注释，`PAGES` 加一行，重新生成。

验收：

- [x] 装了 Litematica 的实例出现投影 tab，没装的不出现；停用插件后 tab 消失。
- [x] 损坏或超大的文件只会让这一项显示「读不了」，不影响整个列表。
- [x] 维护者要用肉眼看：投影列表、详情、材料清单的视觉效果。

### P3 Modrinth 内容源（扩展点：外部服务）

入口条件：P2 已提交。

- [x] T18 先做调研再动手，结果写进实施记录：
  1. 逐个过一遍 core 里提到 Modrinth 的 36 个文件（`grep -rli modrinth crates/lumilio-core/src`），分成三类：**Modrinth API**（要迁进插件）、**`.mrpack` 格式**（`modpack`、`pack_export`，是文件格式，留在 core）、**只是提到这个名字**（不动）。
  2. 读 `discover/query.rs`，把筛选模型（`Pick`、`Stance` 等）和拼 Modrinth URL 的代码分开。
  3. 拿 `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/addon/RemoteAddonRepository.java` 和同目录 `repository/CurseForgeRemoteAddonRepository.java` 对照草拟的接口，在纸面上走一遍，确认 CurseForge 也能实现它。
- [x] T19 在 plugin-api 里定义 `ContentSource`：
  - 能力声明：支持哪些项目类型、哪些筛选、哪些排序。
  - 方法：`search`、`project`、`versions`、`version_files`、`dependencies`。
  - 数据类型由 plugin-api 自己定义，core 负责和现有类型互相转换。
- [x] T20 实现 `ctx.fetch`：只允许访问 `Network.hosts` 里列出的主机；底层走 core 现有的 `Transport`（镜像和代理规则保持不变）；响应大小有上限。
- [x] T21 把 Modrinth API 的部分迁进 `crates/lumilio-plugin-modrinth`。core 的发现、内容安装、更新检查、整合包安装改为通过宿主调用 `ContentSource`。发现页按内容源的能力声明显示筛选项，ADR 0023 的界面和行为不变。
- [x] T22 停用 Modrinth 后：发现页显示「没有可用的内容源」；已安装的内容照常可用；更新检查跳过来源不可用的项，并说明原因。

验收：

- 迁移前后发现页、安装、更新的现有测试全部通过（必要时改为通过宿主注入）。
- `crates/lumilio-core/tests/live_smoke.rs` 照常能跑。
- 停用 Modrinth 的效果有测试覆盖。

### P4 Discord Rich Presence（扩展点：生命周期事件）

入口条件：P3 已提交。

- [ ] T23 在 plugin-api 里定义扩展点 `LaunchObserver`，事件有两种：
  - `Started { instance_name, game_version, loader, target: None | World(name) | Server(address) }`
  - `Exited { outcome, played_seconds }`

  事件在 `service/launch.rs` 里 `launch` / `launch_world` / `launch_server` 确认进程已经起来、以及会话结束写入 `SessionOutcome` 的地方发出。只能观察，不能阻断启动；发事件不能拖慢启动。
- [ ] T24 宿主提供 `Native(DiscordIpc)` 能力，按 D3 只给核心插件。
- [ ] T25 新建 `crates/lumilio-plugin-discord`：参考 `3rd-party/modrinth/packages/app-lib/src/state/discord.rs`；如果从那里改编代码，按 ADR 0022 注明来源，许可证是 GPL-3.0-only。设置项：是否显示游戏名、是否显示世界或服务器（D6）。Discord 没有运行时安静地跳过，不报错。默认关闭（D4）。
- [ ] T26 插件完成后，把整个计划压缩成决策记录（扩展点模型、D1–D7、实际交付），然后删除本文件。

验收：

- 事件的发出时机有测试覆盖：用一个假的观察者，配合现有的启动测试替身。
- 维护者要实测一次：打开 Discord 并启用插件，进游戏能看到状态，退出后状态消失。

## Validation

- 每次提交都通过 pre-commit 的四项检查。新逻辑优先写在 core 和插件 crate 里，并有单元测试；UI 只测渲染器的映射和意图流转。
- 每个插件都要有两类测试：启用时它的贡献出现、停用后全部消失；插件 panic 或超时，只有它自己被停用，启动和其他插件不受影响。
- `plugin_boundaries.rs` 和 `attribution.rs` 一直保持通过。
- 需要维护者肉眼验收的地方，在实施记录里逐条列出。

## References

- 崩溃规则：`3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/CrashReportAnalyzer.java`（51 条规则）。
- 内容源：`3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/addon/RemoteAddonRepository.java`；同目录 `repository/ModrinthRemoteAddonRepository.java`、`CurseForgeRemoteAddonRepository.java`；现有的 `lumilio-core::discover`；ADR 0023。
- Discord：`3rd-party/modrinth/packages/app-lib/src/state/discord.rs`。
- 实例页加 tab 的样板：截图墙的提交 `f15b81e`。
- 扩展模型的先例：Zed 的扩展（同样基于 GPUI；WASM + WIT；不开放任意 UI）、Obsidian 的核心插件开关。

## 实施记录

### 2026-10-04 — P0 开始

- 当前仓库没有 pre-commit 钩子（提交 b86a446 / ADR 0026）；阶段提交前运行 `just check` 的 build、test、clippy、fmt。
- 宿主采用 `PluginHost` / `PluginStatus`；默认调用与清单读取超时 5 秒，搜索阶段另设 20 秒。清单也由工作线程读取，避免注册时在界面线程执行插件代码。
- 从 P0 开始实施；其余两个 in_progress 计划的维护者验收事项保持原状。
- 按维护者要求引入 Longbridge 最新 `gpui-kit` / `gpui-kit-design-guides` 技能（提交 `4c7f1350331562436df868c55ac33bebc4c6406c`），放入现有 `.agents/skills/`，更新 AGENTS 与项目技能引用。上游指导不改变本项目冻结的插件 API、依赖边界或控件设计。
- 边界守卫实际在临时插件依赖 core 时因 `forbidden dependency lumilio-core` 失败，撤掉夹具后 2 项测试通过；别名、workspace、开发/构建及 target 依赖也受检查。NBT 产品代码逐字节不变，7 项原测试迁入新 crate；worlds 测试用新 crate 的公开写入函数造样本。

### 2026-10-04 — P0 验收

- 完成 T1–T7。宿主延迟在工作线程上读取清单，拒绝版本不兼容和重复 ID；同步调用由 spawn_blocking、catch_unwind 与 5 秒超时包围。失败状态本次运行粘住；重新启用不绕过失败，重启才恢复。失效的在途返回值按插件自己的设置修订号丢弃。
- 启用状态、四种字段和恢复默认通过服务保存；单个字段修改合并最新设置，保存失败不发布到宿主，schema 仍为 1。插件 tab 追加为设置索引 5，旧索引不变。空列表、权限人话、技术详情、开关、选择和异步文字/数字弹窗已接入；异步弹窗失败保留草稿，保存时禁止重复提交。
- 自动验证：13 项插件相关 core 测试（含失败/停用后正常启动游戏）、2 项弹窗测试、设置页真实点击/空列表测试、依赖边界与 attribution 均通过。`just ia` 已生成；最终工作树两次 `just check` 均通过（日志 `/private/tmp/lumilio-plugin-p0-check.log`、`/private/tmp/lumilio-plugin-p0-final-check.log`）。默认忽略的真实网络测试没有在 P0 运行。
- 原生视觉检查：任务专用数据目录与临时 app bundle，浅色/深色的空插件列表、插件 tab 切换、hover、720×480 最小窗口、减少动效均已观察；原生退出菜单成功关闭进程。P0 尚无产品插件，有插件的设置行仍需后续阶段结合真实插件再次看。
- 下一步 P1（T8–T12）：迁移五条分析规则并补至少十五条规则；本计划继续保持 in_progress。

### 2026-10-04 — P1 实施中

- P0 已提交为 `19ec0fa`。新增同步 Analyzer、AnalysisInput/GameFacts/ModFact/Finding；Severity 由 API 定义，core 重导出，避免插件引用 core。
- 五条既有匹配条件、顺序与重复匹配去重保持不变，原有测试迁入 crash-analyzer。另参考 HMCL `CrashReportAnalyzer.java` 加入 16 条常见 Mod、配置、运行环境及调试崩溃规则，保留 GPL-3.0-or-later 来源与版权声明；每条有样本正反例。规则顺序保留既有五条在前，新增规则按 Mod/依赖/配置到平台问题排列。
- 诊断报告、实例问题和首页共用宿主汇总。只在最近会话崩溃/启动失败时自动分析；有报告用最新报告，无报告用 latest.log，文件读不了时保留内置检查。GameFacts 由工作线程读安装元数据与启用的 Mod 元数据，Java 主版本取当前有效选择（不声称知道历史进程的 Java）。
- 原生界面沿用原因卡片（Interface，无新动效）：直接显示插件中文标题/建议，证据用既有技术详情折叠。缓存实例页与迟到的结果按启用插件 ID 过滤，切换后重读，防止导航返回恢复停用插件的结果。
- 自动验证已完成：独立分析器 24 项测试、core 分析聚合与服务接线测试、诊断 UI 7 项测试、实际 Backend 注册与启停测试均通过。最终 `just check` 通过（日志 `/private/tmp/lumilio-plugin-p1-final-check.log`）：core 467 passed / 2 ignored，UI 290 passed / 3 ignored，app 9 passed，边界与 attribution 均通过。首轮全量在 Clippy 的多余 `.into()` 上失败，修正后完整重跑通过；真实网络 smoke 测试仍按默认忽略。
- 原生视觉检查使用明确标注的合成报告和任务临时资料：浅色/深色、720×480 最小窗口、常规放大窗口、原因/建议卡片、证据弹窗、减少动效均已查看。实际开关停用后首页问题计数减少、内置问题保留、报告正文可读但原因卡片消失；重新启用并后退到缓存页后卡片恢复。两份临时 app 已用原生菜单退出。
- T8–T12 已完成并可提交阶段代码。真实报告的维护者肉眼验收仍待提供报告路径或实际检查结果，未提前勾选；P1 全阶段尚未验收，P2 不开工。
- 应用 backend 测试移至相邻 `backend/tests.rs` 后完整重跑 `just check` 通过（日志 `/private/tmp/lumilio-plugin-p1-relocated-check.log`），产品行为不变。
- 维护者提供了实例的 logs 目录，其中只有 17 行 `latest.log`：Iris 缺少 Sodium 导致 Fabric 启动失败，旁边没有 crash-reports。将原始日志复制到任务临时实例的日志与报告测试槽，原生查看问题列表、中文原因/建议、17 行正文和证据弹窗均可读；副本不是实际 crash report，不替代该验收。临时应用已退出，用户资料未修改。维护者要求说明如何生成报告：在成功启动的 Java 版测试世界中按住 F3+C 10 秒，等待其生成 crash-reports，再验收真实报告。

### 2026-10-04 — P2 之前需维护者裁决的冻结决定冲突

- D1 明确规定 plugin-api「只依赖 serde」，D5 同时规定 `TabState` 是 `serde_json::Value`。Rust crate 无法在不依赖 serde_json 的情况下公开使用其 Value 类型；借 core/ui 间接引入又违反 D1 的依赖方向。
- 建议只在 D1 的 plugin-api 依赖清单中允许 `serde_json`，保留 D5 的 TabState 语义，其余边界不变。暂不修改冻结决定，不开始 T13；待维护者确认，并先完成 P1 的真实报告视觉验收。
- 维护者随后裁决「允许增加 serde_json」。D1 已据此修订，P2 实施时可为 plugin-api 增加该依赖；其余冻结决定保持原样。P1 的真实报告视觉验收仍待完成。

### 2026-10-04 — P1 真实报告视觉检查

- P1 产品代码已提交为 `78d4071`。维护者生成 `crash-2026-10-04_18.24.41-client.txt`，内容确为 Minecraft/Fabric 的 `Manually triggered debug crash`。
- 原始报告仅复制到任务临时实例。浅色和深色、720×480 最小窗口均已在原生应用中查看：只出现一条 Info 级调试崩溃原因，中文建议为重新启动，报告正文与证据弹窗均可读。没有把手动崩溃误报为 Java、内存或 Mod 故障。
- 维护者查看后确认「通过提交 p1，暂时暂停」。最后一项验收已勾选，P1 全阶段完成；提交本次验收记录后暂停目标，不开始 P2。恢复时从 T13 开始，D1 增加 serde_json 的裁决已记录在上。

### 2026-10-04 — P2 实施中（T13–T17 代码完成，待全量检查与肉眼验收）

- 恢复 P2：维护者在 P1 验收后只是暂停，随后再次要求推进本计划。D1 的 serde_json 裁决已落实到 plugin-api。
- 留给实施者的细节：`HostContext` 增加 `list_files(dir)`（递归列出授权目录下的文件，相对游戏目录；不跟随符号链接，最多 5000 项、深度 8）。Litematica 要枚举 `schematics/**/*.litematic`，只有 `read_file` 做不到；它和 `read_file` 走同一套 `ReadGameFiles` 校验（拒绝 `..`、绝对路径、符号链接，单文件上限 32 MiB），所以没有扩大权限面。
- 实例页 tab：内置 tab 的常量和 `LUMILIO_PAGE` 行为不变；插件 tab 只存在于「显示位置」里，插在「截图」之后、「历史」之前。`tab` 仍是内置序号，另有 `plugin_open` 记录打开的插件 tab，`select_tab` 会清掉它。`kit::tabs`/`PortTabs` 改为接受 `Into<SharedString>`，原调用点不变。
- 视图树渲染器只有一份（`instance_detail/plugin_tabs.rs`），每个 `View` 对应 kit 组件；Detail 里的 `Key` 统一排在末尾；RGBA 在渲染时转 BGRA，尺寸与像素数不符的图片直接丢弃。`destructive` 的键先弹确认框再发出动作。`SaveAs` 的文件名只保留最后一段，`RevealGameFile` 必须落在授权目录内且文件存在，否则宿主丢弃该 Effect。
- 宿主：`PluginHost::{tabs, tab_view, tab_action}`，每个「实例 + 插件」的 `TabState` 由宿主保存；服务方法 `plugin_tabs / plugin_view / plugin_action`。动作执行后，app 逐个执行 Effect，再重新读取视图。
- Litematica：出现条件为 mods 里有 id `litematica`，或 `schematics/` 下已有文件（插件只能列出文件，所以空文件夹不算）。列表读取有 3 秒预算，超出的文件只显示文件名；单个文件读不了只让那一行写「读不了」。材料清单按格式规范自行实现（位宽 `max(2, ⌈log₂ 调色板长度⌉)`，值可跨两个 long），缺失或越界的调色板索引、过短的 BlockStates 都是错误而不是 panic。CSV 带 UTF-8 BOM。
- 中途磁盘写满（`target` 188G），删除了可再生的 `target/debug/incremental`（58G）后继续；没有动源码和其他产物。
- 需要维护者肉眼看：投影列表（缩略图、名称、作者与尺寸、方块数）、详情（事实、材料清单表格）、关闭插件后 tab 消失、导出 CSV 的保存对话框。

### 2026-10-04 — P2 验收

- 维护者用真实的投影文件看过投影 tab，确认「P2 验收成功」。看图后的两条修改已落实：列表标题改为文件名（子文件夹显示为标签，`Metadata.Name` 常是 Litematica 写的默认值 "Unnamed"，移到详情的「投影名称」）；详情的操作按钮排在标题下面，材料清单改成最高 360 px 的滚动容器（表头固定，滚轮留在容器内，做法同诊断日志）。
- P2 全阶段完成，P3 尚未开始。3D 预览不在本计划范围内，另开 `litematica-3d` 计划；P3 可以与它并行排期。

### 2026-10-04 — P3 调研与基础接口（T18–T20）

- 工作分支 `feat/plugin-content-source`，worktree `.worktrees/plugin-content-source`，基于已验收 P2 的 `c9d41e9`。主分支的后续 3D 工作引用尚未跟踪的 schematic-render crate，本轮不纳入。
- T18 逐个核对后是 37 个文件（计划中的 36 是旧计数），按主职责分组；混合文件只迁 API 调用：
  - **API 与接线（23）**：`discover/{client,error,kinds,mod,project,query,search,tags,versions,tests}.rs`；`service/{content,content_install,discover,modpacks,worlds,types}.rs`；`updates.rs`、`updates/tests.rs`、`content_sources.rs`、`lib.rs`；`service/tests/{content,discover,packs}.rs`。`kinds` 的目录映射、`versions` 的兼容选择、`content_sources` 的列表组装仍属 core；`service/modpacks` 的本地导入和 `service/worlds` 的格式导出也仍属 core，只替换其中的远程查询。
  - **mrpack 格式（7）**：`modpack.rs`、`modpack/tests.rs`、`pack_export.rs`、`pack_export/tests.rs`、`service/packs.rs`、`service/tests/diagnostics.rs`、`service/tests/mod.rs`。格式、可信 CDN、zip 夹具保留；导出前的识别请求在上组的 `service/worlds` 迁移。
  - **名称 / 来源 / 通用行为（7）**：`content.rs`、`instance.rs`、`servers/ping.rs`、`plugins/tests.rs`、`transfer/mod.rs`、`tuning.rs`、`discover/version_groups.rs`。哈希、实例来源字段、服务端 ping 的 attribution、权限测试、UA、筛选偏好与版本范围展示保留。
- `discover/query.rs` 的 `Stance`、`Pick`、SearchQuery 字段是用户意图；`expression/url/literal/one_of/none_of/environment_groups` 是 Modrinth v3 协议。基础契约另定义语义查询，T21 才把协议生成迁进插件并做 core 转换，保留 ADR 0023 的 include/exclude、any/all 与高级排除语义。
- 对照 HMCL 的 `RemoteAddonRepository.java` 和 `repository/CurseForgeRemoteAddonRepository.java`（只研究契约，不复制实现）：项目 ID、版本 ID 都是不透明字符串；CF 的 project/file 路径要求版本引用同时含项目 ID。按类型声明筛选及排序，CF 可声明单个游戏版本、单个分类、单个 loader，而不承诺 Modrinth 的排除/组合/环境等筛选。搜索分页对应 index/pageSize，下载数/创建/更新排序可映射 sortField；files 转版本和文件，relations 转依赖，downloadUrl 缺失则没有可安装文件。识别允许 SHA-1 或 Murmur2 指纹（由宿主算，插件不读任意本地文件）；批量识别/兼容更新、标签及作者摘要也需有契约，避免现有安装/更新/导出功能遗漏。CF 的 X-API-KEY 可通过声明式设置读取并作为请求头交给宿主；本轮不实现 CF。
- 网络细节：宿主校验插件提交的原始 URL 的精确主机，再应用维护者配置的 SourceChain；镜像是宿主的可信替代，不给插件任意访问镜像的权限。重定向必须逐跳检查，不能依赖 reqwest 默认自动跳转；JSON POST 不镜像重试，保持原 client 的规则。响应与请求体均有大小上限；超时使网络 future 取消，不让已失败的 worker 一直等网络。
- 本批完成 T18–T20；T21–T22 的产品迁移与界面接线另批推进。

### 2026-10-05 — P3 基础实现与验证

- 实现 `content` 契约与 `SearchQuery::validate`：按项目类型限制排序、筛选数量、排除、any/all 与高级筛选；`VersionRef` 包含项目和版本 ID；文件识别输入是宿主计算的指纹与关联 key，不向插件交出本地路径。数据协议可 serde 往返，plugin-api 没有增加 serde / serde_json 之外的依赖。
- 宿主提供 `content_sources / call_content / search_content`，能力声明也在 worker 隔离边界内读取；普通调用仍为 5 秒，搜索独立预算 20 秒，网络与该次插件调用共用期限。
- `ctx.fetch` 保持 GET 接口，新增 `ctx.request(FetchRequest)` 承载 POST、请求头与请求体，兼容旧上下文的默认 GET 实现。请求体上限 1 MiB、请求头合计 16 KiB、响应 32 MiB（同时检查 Content-Length 和流式累计），最多 10 次重定向；拒绝路由/代理/分帧头、URL 凭据、非 HTTP(S) URL，跨 origin 跳转去掉敏感头，拒绝 HTTPS 降级。
- `Transport::send_no_redirect` 默认拒绝；产品 DefaultTransport 使用相同默认代理/TLS/UA 配置的无自动跳转 HTTP client，由宿主逐跳授权。`HttpTransport::from_client` 的不透明自定义 client 保留原行为，但新的受限请求拒绝执行，防止偷偷丢失其代理配置或绕过重定向检查；此限制有测试。
- 镜像持久化成功才发布，下一次调用用新快照；手工写坏镜像配置只使插件网络调用返回错误，不阻止服务启动或打开本地库。正在使用旧快照的调用不会被换掉传输路径。
- 权限守卫已证明会失败：临时跳过原始主机授权检查后，定向测试因 `https://evil.test/` 返回 HTTP 200、预期 PermissionDenied 而变红；已还原检查并重跑。42 项插件相关测试通过，包含启停、panic/超时隔离、搜索预算、网络 future 取消、响应大小、逐跳授权、镜像持久化和配置损坏。原生 HTTP 单跳/UA 与不支持受限请求的 transport 测试也已通过首轮，最终全量将再次覆盖。
- 环境记录：共享 target 与另一个主工作区构建争锁，两个过滤后的测试进程曾停在 macOS dyld 启动阶段，随后正常退出。为 worktree 建了独立缓存；全量复制 split-DWARF 元数据过慢，停止后改为只克隆依赖库、Cargo fingerprints 和 build 产物。停止的仅是本任务的复制/等待进程，另一项构建未被操作。
- 两轮完整 `just check` 都通过（日志 `/private/tmp/lumilio-plugin-p3-foundation-check.log`、`/private/tmp/lumilio-plugin-p3-foundation-final-check.log`）：最终 core 496 passed / 2 ignored，UI 296 passed / 3 ignored，app 10 passed，边界与 attribution 均通过；新添 24 项测试。真实网络 live_smoke 的 2 项仍按默认忽略，本批尚未迁移远程 API，不声称完成真实 Modrinth 链路验收。
- 搜索筛选语义沿用 core 的 Modrinth App 来源，API 模块保留了 `packages/ui/src/utils/search.ts` / GPL-3.0-only / ADR 0022 attribution，补齐注释后已完整重跑检查。本批没有 UI 改动或新的肉眼验收项。T18–T20 已完成；下一批从 T21 迁移 Modrinth 产品调用和 core 类型转换，再做 T22 的停用反馈。P3 尚未验收，P4 不开始。

### 2026-10-05 — T21 独立 Modrinth 插件

- 在同一 worktree 从 `dfacd47` 继续，先交付 T21 的插件实现；T21 整项仍未完成。新增平铺的 `lumilio-plugin-modrinth`，只依赖 plugin-api、serde、serde_json、url，没有网络客户端或文件访问。清单 ID 为 `lumilio.modrinth`，默认启用，只申请 `api.modrinth.com` 网络权限。
- 将原 `discover/{query,search,project,tags,versions}.rs` 的协议实现抽取到插件，直接输出 plugin-api 的内容契约；原 core 实现暂留供现有产品调用，下批完成适配与切换后删除。没有在 app 注册插件，因此本批不会出现一个尚不能停用旧 API 的设置开关，也没有改变发现页、安装或本地内容行为。
- 完整实现搜索、项目详情、版本与 changelog、指定版本文件和依赖、筛选选项、SHA-1 识别、兼容更新、项目摘要与团队作者。所有 GET / JSON POST 经 `ctx.request`，HTTP 非 2xx 与必需数据解码失败返回错误；作者与 loader 列表失败保留已有页面 / 选项。识别按宿主 correlation key 返回，合并重复哈希但保留每个 key，忽略远端未请求的哈希，不把空批次发到网络。
- 搜索继续参考主工作区只读的 `3rd-party/modrinth/packages/ui/src/utils/search.ts`（`newFilters`、`getEnvironmentFilterGroups`、`formatSearchFilterValue`），保留 GPL-3.0-only / ADR 0022 来源声明；版本查询对照 HMCL 的 `repository/ModrinthRemoteAddonRepository.java`。worktree 未初始化第三方子模块，读取的是主工作区副本，没有修改它们。
- 插件按类型验证能力，不再把不支持的筛选悄悄丢掉。契约中的 loader `any/all` 分别编码为 OR / AND；下一批 core 适配必须把原 core 固定 OR 的 loader 选择归一为 `any = true`，保持 ADR 0023 的现有界面语义。项目 / 版本 ID 按单个 URL path segment 编码，拒绝空值和特殊点段；指定版本返回的项目及版本 ID 必须与引用一致。
- 27 项独立插件测试覆盖原协议样本与上下文请求，新加宿主测试验证真实插件的贡献、停用后无请求、缺少授权无 I/O，以及 HTTP 429 后本次运行保持 Failed；core 对插件的依赖仅是 dev-dependency。临时去掉项目 ID 校验后，版本文件测试编译成功并因错误项目仍返回文件而失败；校验已还原，27 项测试重跑通过。另增加默认忽略的只读真实 API 契约测试，不下载游戏、不接触用户启动器资料。
- 验证：27 项插件单元测试、3 项真实插件宿主测试、手动运行的只读真实 API 契约测试全部通过。真实测试在 3.77 秒内读完搜索、项目、带 changelog 的版本、指定版本文件 / 依赖、识别、兼容更新、项目摘要、标签列表，没有下载游戏或修改用户资料；该项默认忽略。最终 `just check` 通过（日志 `/private/tmp/lumilio-plugin-modrinth-source-check.log`）：core 499 passed / 3 ignored、UI 296 passed / 3 ignored、app 10 passed，边界 / attribution、Clippy 和格式检查均通过。原 `live_smoke` 两项仍按默认忽略，不声称已验证产品迁移后的安装 / 启动。
- 首轮测试夹具编译曾使用 API 不存在的 `Version::required_dependencies` 和 `SourceChain::official`，已分别改为直接筛选依赖和构造 OfficialSource 链；这些编译失败不是回归测试变红的证据。
- 更新实施记录后 `just docs` 通过（日志 `/private/tmp/lumilio-plugin-modrinth-source-docs.log`）。没有产品界面改动，本批无新的原生视觉验收项；插件清单文案的实际设置行将在 app 注册时一起查看。
- 下一批从 core 的类型转换与调用适配开始：切换发现 / 内容安装 / 更新 / 整合包查询，删除旧协议实现；缓存结果也必须按插件启用状态过滤，不能用缓存绕过停用。app 注册、原 service / live_smoke 测试注入、能力驱动的筛选和 T22 的反馈一起完成。D2 的 Failed 本次运行粘住，旧过滤器「网络恢复后同一 service 重试成功」测试需要按此冻结行为调整。P3 不验收，P4 不开始。

### 2026-10-05 — T21–T22 core 切换到内容源插件

- core 不再有 Modrinth 协议代码：删除 `ModrinthClient`、搜索 URL / 过滤表达式生成和各类 JSON 解码，只留启动器自己的数据类型（`Project`、`Version`、`SearchQuery` 等）。新增 `discover::{source, convert}`：`PluginHost::content_client()` 取第一个启用的内容源，`ContentClient` 提供与旧客户端同名的方法（搜索、项目、版本、识别、兼容更新、摘要、筛选项），类型在 core 与 plugin-api 之间转换。发现、内容安装、依赖报告、版本切换、整合包安装、内容来源识别、整合包导出的识别都改走它。
- ADR 0023 的语义保持：已选加载器统一以 `any = true` 交给插件；资源包不带加载器，只有 Mod / 整合包带运行环境；页面大小限制 1–100。排序不被内容源支持时退到它的第一项，避免插件拒绝整个搜索而被停用。`LoaderTag` 加了 `other_kinds`，保留 `plugin`、`datapack` 之类非 `ProjectKind` 的类型，加载器筛选结果不变。
- `ProjectSummary.team` 改为 `author`，不再有单独的团队查询。项目作者由 `Project.author` 提供。
- 发现页筛选栏按 `KindAbilities` 显示：内容源不支持的分区（版本、加载器、环境、许可证、高级排除、隐藏已安装）不出现；能力未知时全部显示。
- T22：停用后搜索、筛选、详情、安装、整合包安装都返回 `ServiceError::NoContentSource`，发现页显示「没有可用的内容源」并指向设置 › 插件；筛选缓存先问内容源再用，停用后不会从缓存返回。已安装内容照常列出，`ContentList.source_note` 写明来源信息问不到的原因（没有内容源、已停止、没有回答），内容页把原因显示出来。`updates::check` 在内容源不可用时整体失败并带原因，不会报告「全部最新」。
- 按冻结的 D2，内容源一次出错（包括网络错误、HTTP 非 2xx）就进入 Failed，本次运行内保持停止，设置页和发现页会说明原因。旧的「网络恢复后同一 service 重试成功」测试改为「失败后保持停止并说明原因」。这对内容源偏严（一次网络抖动就要重启才能恢复），是否放宽 D2（例如把可恢复的网络错误和插件 bug 区分开）需要维护者裁决，本批没有改动。
- app 注册 `lumilio-plugin-modrinth`；service 测试和 `live_smoke` 注入它，测试传输加了 `send_no_redirect`。新增覆盖：搜索经内容源并转换类型、停用后各入口返回 NoContentSource 且不发请求、失败后保持停止、能力驱动的筛选分区、加载器其他类型保留、更新检查在内容源停用时失败。
- 验证：本机只跑了受影响的 core 测试（90 通过、1 忽略），其余按维护者要求交给 GitHub Actions 的 `just ci`。需要肉眼看：发现页筛选栏（Modrinth 下应与之前完全一致）、停用 Modrinth 后的发现页空状态和内容页提示。
- 维护者裁决放宽 D2（CI 的「失败的下载可以重试」测试暴露：断网后内容源进入 Failed，重试永远不会成功）：新增 `PluginError::Transient`，表示外部世界不配合（连不上、HTTP 非 2xx），只让这一次调用失败，插件保持启用并可重试，原因可由 `last_transient_error` 读到。panic、超时、越权、非法输入、重定向 / 大小超限等仍按 D2 停用到重启。Modrinth 的 HTTP 非 2xx 与 `ctx.request` 的传输错误使用它。上面「内容源一次出错就 Failed」的说明以此为准；旧的「网络恢复后同一 service 重试成功」测试因此保持原有含义。

### 2026-10-05 — P3 验收

- 维护者肉眼确认：发现页筛选栏在 Modrinth 下与之前一致；停用 Modrinth 后发现页空状态「没有可用的内容源」、内容页来源提示可见；设置 › 插件中的 Modrinth 行正常。
- P3 全阶段完成；P4 入口条件（P3 已提交）已满足，但尚未开始。
