# 0007 — 平台 App State 根目录与生命周期

- Status: proposed
- Date: 2026-09-30（2026-10-03 并入原 `docs/app-state.md`）

## Context

ADR 0006 已建立 SQLite、独立 profiles 和共享 meta。现有单根解析尚未区分缓存、配置与恢复状态；Windows 使用 Roaming AppData；natives 没有平台维度。需要明确离线依赖、用户世界与可丢弃缓存的边界。

## Decision

app 解析平台 data/config/state/cache 四个根，core 的 `Layout` 接收这些路径，统一定义所有应用自有路径；core 不读取平台环境。沿用 ADR 0006 的数据库、profile 和 meta；已安装的依赖不按缓存清除。在线秘密只进系统凭据库（ADR 0020）。下面是目标契约，分 P1/P2/P3 实施；实施前不改变现有读写行为，也不宣称新目录已存在。

### 状态的边界

磁盘上是持久事实，进程内是当前状态；磁盘不是 UI 状态的直接镜像。

| 层 | 权威来源 | 重启后行为 |
|---|---|---|
| 实例目录、收藏、集合 | launcher.db | 读取恢复；目录缺失标记问题，不删除记录 |
| 全局默认设置、账户公开信息 | settings.json | 读取恢复；实例设置覆盖全局默认 |
| 世界、模组、游戏配置 | profiles/<id>/game 里的实际文件 | 按需扫描；数据库不能覆盖游戏改动 |
| 变化、游戏会话与快照 | 实例 history.jsonl / snapshots | 保留；历史不是当前文件状态的替代品 |
| 已安装游戏、加载器、Java | meta / runtimes 与校验结果 | 支持离线启动；不能按普通缓存淘汰 |
| 可恢复操作 | state/operations 里的操作日志 | 对照文件恢复或标记 interrupted，不直接重放副作用 |
| 启动进度、进程句柄、任务取消、网络响应 | core 服务内存 | 重新探测；PID 单独不能证明游戏仍在运行 |
| 当前路由、展开项、焦点、滚动 | UI 内存 | 不持久化；window.json 只存窗口尺寸和位置 |
| 在线账户秘密 | 系统凭据库 | 重新读取；失败时请求登录，不降级写明文 |

### 平台路径

| 角色 | macOS | Windows | Linux |
|---|---|---|---|
| data | ~/Library/Application Support/LumilioCL | %LOCALAPPDATA%/LumilioCL | ${XDG_DATA_HOME:-~/.local/share}/lumilio |
| config | data/config | data/config | ${XDG_CONFIG_HOME:-~/.config}/lumilio |
| state | data/state | data/state | ${XDG_STATE_HOME:-~/.local/state}/lumilio |
| cache | ~/Library/Caches/LumilioCL | data/cache | ${XDG_CACHE_HOME:-~/.cache}/lumilio |

- Windows 用 Local 而不是 Roaming：游戏资源、Java、实例路径都是本机的，而且可能很大。
- 优先级：非空 `LUMILIO_HOME` → 平台 API / 有效的 XDG 变量 → 平台默认。`LUMILIO_HOME` 本身就是 data 根，config/state/cache 放在它的子目录里（测试与便携模式）。明确设置却无效时报错，不悄悄回落；XDG 的空值或相对路径按规范忽略；绝不使用当前工作目录。
- 沙盒分发由系统 API 返回容器内的目录。

### 目标目录树

```text
data/
  launcher.db                     实例与集合；schema 由 user_version 管理
  storage.json                    布局版本、数据根身份、迁移完成记录
  config/settings.json            全局默认、源配置、账户公开信息
  state/
    activity.jsonl                已完成的活动
    window.json                   窗口几何；跨屏恢复时限制到可见区域
    operations/<operation-id>/    可恢复操作的日志与提交状态
    logs/  recovery/  locks/      启动器日志；损坏文件与迁移报告（不自动清）；OS 级锁
  profiles/<instance-id>/
    game/                         Minecraft 和模组自己管理的文件，启动器不改名
    history.jsonl                 实例变化与游戏会话
    snapshots/<snapshot-id>.zip   用户可恢复的备份
  meta/
    versions/<release-id>/  libraries/  assets/indexes/  assets/objects/  log_configs/
    natives/<platform-key>/<release-id>/
  runtimes/<platform-key>/<runtime-id>/
cache/
  http/  images/  downloads/<operation-id>/  staging/
```

目录按需创建。platform-key 是规范化的 OS/CPU 标识（必要时加 ABI），不同平台不共用 natives 或 Java。`meta` 是共享的已安装依赖，不是「清缓存」的目标；实例的可变文件不与共享文件做可写硬链接。

### 写入、删除与恢复

- launcher.db：schema 迁移前做一致备份；遇到更新的 schema 拒绝写入；损坏时隔离原件并进入恢复模式，不用空库覆盖。
- 小 JSON：同目录临时文件 → 刷新 → 原子替换；读不懂的新版本文件保持原样。JSONL 逐行容错并报告跳过的行数。
- 下载先校验 size/hash 再提交。原子重命名只用于同一文件系统；跨卷时先复制到目标旁的临时文件、校验，再提交。
- 操作日志记录操作 id、目标实例、输入与校验信息、阶段和提交结果，不记录秘密。恢复时对照当前文件，已完成的步骤不重复覆盖用户内容。
- 一个 data 根只允许一个写入进程（ADR 0009）；共享资源按资源键加锁。删除实例和回收共享资源分别见 ADR 0017 及删除流程。
- 清空 cache 后，启动器能重建展示缓存，已完整安装的游戏仍可启动。世界、快照、已安装的 Java 和 meta 永远不会被清。
- 初始保留策略（待实现）：应用日志 14 天且不超过 100 MiB；已完成活动最多 5,000 条；HTTP/图片缓存 LRU 512 MiB；失去操作引用的临时下载 7 天后清除。

### 启动与备份

启动顺序：解析路径 → 获取写锁 → 检查布局/schema → 恢复未完成的迁移和操作 → 打开 store → 发布库快照 → 后台扫描。扫描和磁盘 I/O 不在 UI 线程。备份见 ADR 0015：cache、锁、临时工作区和凭据不导出；数据库用 SQLite 的备份能力，不在写入时只复制主文件。

### 实施阶段

| 当前 | 目标 | 阶段 |
|---|---|---|
| `backend::data_root` 返回单根；Windows 用 APPDATA | 平台解析出四个根；Windows 用 Local | P1 |
| 部分模块直接 join 路径 | 所有应用自有路径都经过 `Layout` | P1 |
| 根目录下的 settings.json / activity.jsonl / downloads | config/、state/、cache/ 下 | P2 |
| natives/<release>；托管 Java 不分平台 | 按 platform-key 隔离 | P2 |
| 内存任务板与完成日志 | 可恢复的操作日志、资源锁 | P3 |
| 损坏的 DB 隔离后启动空库 | 保留数据并进入恢复模式 | P3 |

- P1 验收：模拟三平台路径与覆盖优先级（缺 HOME、相对 XDG、非 ASCII 路径）；所有路径受 `Layout` 控制；core 测试不依赖 GPUI。
- P2 验收：保留现有 profiles/meta 和 release id；同时检测 Windows 旧根和新根，两边都有库时不自动合并。迁移独占执行、记录阶段、复制并校验后再切换，成功前保留源数据，中断后可以重新进入。
- P3 验收：注入提交前后崩溃、重复启动、删除失败、cache 全清、DB 损坏等场景，证明不覆盖世界、不重复副作用、已完整安装的游戏仍可离线启动。

## Consequences

- Positive: 备份和清理的语义可预测；大型本机资源不漫游；故障恢复有明确依据。
- Negative / trade-offs: 多根 `Layout` 增加了参数和迁移复杂度；缓存丢失会让下载从头开始。
- Follow-ups: P1/P2/P3 各开一个计划，实施时由维护者把本 ADR 转为 accepted。自定义实例数据根、远程同步不在首轮范围内。
