# 0031 — 插件宿主与四个核心插件

- Status: accepted
- Date: 2026-10-05

## 背景

不是每个人都需要的功能、持续增长的分析规则和独立演进的外部服务需要可开关的插件宿主。用崩溃分析、Litematica、Modrinth、Discord 四个核心插件验证扩展点；实例存储、安装下载、Java、账户、启动和锁仍由 core 负责。WASM、社区插件、插件市场与从磁盘加载插件不在本计划内。

## 决定

扩展点为 `Analyzer`（纯数据分析）、`InstanceTab`（实例页内容）、`ContentSource`（内容服务）和 `LaunchObserver`（只能观察生命周期，不能阻断或拖慢启动）。保留 D1–D7，并纳入维护者的两次修订：

- **D1 依赖边界**：平铺的 `lumilio-plugin-*` crate 只依赖 plugin-api、独立的 `lumilio-nbt` 与通用库，不依赖 core、UI、app 或 GPUI；core 拥有宿主，UI 渲染协议，app 注册插件。plugin-api 只依赖 `serde`、`serde_json`；维护者明确允许新增 `serde_json`，以支持 D5 的 `TabState`。docgen 测试守住依赖边界。
- **D2 同步 API 与隔离**：插件同步调用由宿主通过 `spawn_blocking`、`catch_unwind` 和超时隔离，默认 5 秒、内容搜索 20 秒；网络与文件操作走宿主上下文。失败进入本次运行粘住的 `Failed`，重启恢复。维护者后来允许 `PluginError::Transient` 表示外部 / 网络失败：只失败这一次调用，不进入 `Failed`，可重试；panic、超时、越权等仍停用到重启。
- **D3 清单与权限**：`Manifest` 声明唯一 ID、名称、描述、版本、API 版本、默认启用、权限与设置；宿主拒绝不兼容 API，每次上下文调用检查游戏目录只读范围、精确网络主机或启动事件权限。`Native(DiscordIpc)` 只给核心插件；设置页用人话解释权限。
- **D4 默认启用**：分析器、Litematica、Modrinth 默认启用；向第三方披露状态的 Discord 默认关闭。启用覆盖与设置保存在 `LauncherSettings.plugins`，缺省取清单值；新增字段不升级 settings schema，也不弹检测模组后的启用提示。
- **D5 内容视图树**：插件描述内容，不提供布局、颜色或绘图原语；UI 使用一个通用渲染器。Elm 式 `view/update` 与宿主按「实例 + 插件」保存的 `TabState` 驱动交互；文件定位、另存为、提示由声明式 `Effect` 交给宿主授权执行，破坏性操作统一确认。
- **D6 声明式设置**：Toggle、Choice、Text、Number 四种字段由宿主统一渲染、校验、持久化和恢复默认；插件在下一次调用读取新设置。
- **D7 IA**：插件视图的用户路径在插件 crate 中以 `ia[plugin.<短名>]` 声明并注册 docgen 页面；开关、权限和设置表单在 UI 中归入 `ia[settings]`，生成 `docs/ia/paths/`。

## 已交付

| 阶段 | 交付 | 提交 |
|---|---|---|
| P0 | 宿主、设置、权限与隔离、独立 NBT crate、依赖守卫 | `19ec0fa` |
| P1 | crash-analyzer，诊断报告与问题列表汇总 | `78d4071` |
| P2 | 实例插件 tab、通用视图树、Litematica 元数据与材料清单、CSV 导出 | `6a26e31` |
| P3 | ContentSource、受限网络、独立 Modrinth 插件与产品调用迁移；Transient 修订 | `dfacd47`、`6965c2a`、`7fff4c9`；`8e681b6` |
| P4 | LaunchObserver、宿主 Discord IPC 与 Discord 插件；官方默认 App ID | `937d763`；`f011e63` |

## 验收

- P0：空插件设置页与假插件隔离测试通过，覆盖启停、panic、超时、越权和 API 不兼容；依赖守卫证明能失败。
- P1：原五条规则与新增规则测试通过，启停贡献受测试覆盖；维护者验收真实的手动崩溃报告显示。
- P2：维护者用真实投影验收列表、详情和材料清单；出现条件、停用后 tab 消失及损坏 / 超大文件隔离均已验收。
- P3：内容协议与宿主授权测试通过；维护者确认 Modrinth 筛选栏与原来一致，停用后的发现页空状态、内容页来源提示和设置行正常。
- P4：生命周期、授权、隔离与清除测试通过。2026-10-05 维护者在 Linux debug 构建 `f011e63` 实测通过：Discord 打开、插件启用、ID 留空（官方默认 `1556820934805954691`）；启动原版实例「原版 26.3」，手机 Discord 个人资料显示「Playing LumilioCL — 原版 26.3 · Minecraft 26.3 · Vanilla」，带已用时间与应用图标；退出游戏后状态清除。

## 影响与后续

插件可以独立开关，宿主隔离故障与权限，启动链路保持在 core。代价是同步 worker、协议转换和通用视图树的维护成本；声明式界面限制自由布局，超时也不能强制终止已经运行的同步代码。

Litematica 3D 预览是独立计划的成果，见 ADR 0027–0029，不属于这里的 P2。WASM 社区插件尚未开始；Discord Windows named-pipe 未在本次实机验收中验证。P0–P4 已验收，原计划关闭并删除。
