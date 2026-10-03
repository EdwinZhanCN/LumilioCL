# LumilioCL App State 规划

本文是目标存储契约，尚未全部实现；当前基线见 [storage behavior](behavior/storage.md)，决策提案见 [ADR 0007](../.agents/decisions/0007-platform-app-state.md)。延续 ADR 0006 的 SQLite、实例隔离和共享 meta，不新增 ARCH 外功能。

用户入口、业务流程和开发对照见 [workflows](workflows/README.md)；本规范对应的取消、提交、重启与恢复验收见 [状态与恢复](workflows/state-and-recovery.md)。流程文档不改变本文的规划/实现状态。

## 1. App State 的边界

App State 包含磁盘上的持久事实和进程内的当前状态。磁盘不是 UI 状态的直接镜像。

| 层 | 权威来源 | 重启后行为 |
|---|---|---|
| 实例目录、收藏、集合 | launcher.db | 读取恢复；目录缺失标记问题，不删除记录 |
| 全局默认设置、离线账户 | settings.json | 读取恢复；实例设置覆盖全局默认 |
| 世界、模组、游戏配置 | profiles/<id>/game 的实际文件 | 按需扫描；数据库不能覆盖游戏改动 |
| 变化、游戏会话与快照 | 实例 history.jsonl / snapshots | 保留；历史不是当前文件状态的替代品 |
| 已安装游戏、加载器、Java | meta / runtimes 与校验结果 | 支持离线启动；不能按普通缓存淘汰 |
| 活动恢复记录 | state/operations 中的操作日志（规划） | 对照文件恢复或标记 interrupted，不能直接重放副作用 |
| 启动进度、进程句柄、任务取消、网络响应 | core 服务内存 | 重新探测；PID 单独不能证明游戏仍运行 |
| 当前路由、展开项、焦点、滚动 | UI 内存 | 默认不持久化；window.json 仅保存窗口尺寸/位置 |
| 在线账户秘密 | 系统凭据库（未来认证阶段） | 重新读取；失败请求登录，不降级写明文 |

## 2. 平台路径

路径以逻辑角色传入 core；由 app 解析平台位置。以下为普通桌面分发默认值；沙盒分发由系统 API 返回容器内目录。

| 角色 | macOS | Windows | Linux |
|---|---|---|---|
| data | ~/Library/Application Support/LumilioCL | %LOCALAPPDATA%/LumilioCL | ${XDG_DATA_HOME:-~/.local/share}/lumilio |
| config | data/config | data/config | ${XDG_CONFIG_HOME:-~/.config}/lumilio |
| state | data/state | data/state | ${XDG_STATE_HOME:-~/.local/state}/lumilio |
| cache | ~/Library/Caches/LumilioCL | data/cache | ${XDG_CACHE_HOME:-~/.cache}/lumilio |

Windows 使用 Local 而非 Roaming：游戏资源、Java、实例路径具有本机属性，且可能很大。不引入配置漫游服务。

优先级：非空 LUMILIO_HOME → 平台 API/有效 XDG 变量 → 平台默认。LUMILIO_HOME 保留现有语义：它本身就是 data 根，config/state/cache 放其子目录，方便测试与便携模式。明确设置却无效时报告错误，不悄悄回落。覆盖路径须为绝对路径；XDG 空值或相对值按规范忽略。禁止静默使用当前工作目录。

依据：[Apple 文件系统用途](https://developer.apple.com/documentation/foundation/using-the-file-system-effectively)、[Microsoft Known Folders](https://learn.microsoft.com/en-us/windows/win32/shell/knownfolderid)、[XDG Base Directory](https://specifications.freedesktop.org/basedir/0.8/)。本表是 LumilioCL 的具体选择。

## 3. 目标目录树

以下 macOS/Windows 树中 config、state 是子目录；Linux 为上一表的独立根。

```text
data/
  launcher.db                     实例与集合；schema 由 user_version 管理
  storage.json                    布局版本、数据根身份、迁移完成记录
  config/
    settings.json                 全局默认、源配置、账户公开信息
  state/
    activity.jsonl                已完成活动记录
    window.json                   窗口几何信息；跨屏恢复需限制到可见区域
    operations/<operation-id>/    可恢复操作的日志与提交状态
    logs/                         启动器日志；游戏日志仍在实例 game/logs
    recovery/                     损坏文件、迁移报告；不自动清除
    locks/                        单写者锁及操作锁；锁由 OS 机制实现
  profiles/<instance-id>/
    game/
      mods/ resourcepacks/ shaderpacks/
      saves/ config/ logs/ crash-reports/ screenshots/
      options.txt servers.dat ... Minecraft/模组自行管理的其他文件
    history.jsonl                 实例变化与游戏会话
    snapshots/<snapshot-id>.zip   用户可恢复备份
  meta/
    versions/<release-id>/        原始/组合版本 JSON、客户端 JAR
    libraries/                    按 artifact 坐标组织的共享依赖
    assets/indexes/                资源索引
    assets/objects/                按上游内容 hash 组织的资源
    natives/<platform-key>/<release-id>/  OS + CPU + 版本隔离的解压结果
    log_configs/                  启动所需的日志配置
  runtimes/<platform-key>/<runtime-id>/   启动器安装的 Java

cache/
  http/                           带 ETag、过期时间、来源的请求缓存
  images/                         图标、封面及缩略图
  downloads/<operation-id>/       下载分片/整合包临时归档
  staging/                        可重新创建的校验/解压工作区
```

这是规划树，不要求启动时创建所有目录。按需创建；没有在线账户 token 文件。版本身份包含游戏版本、加载器种类与版本；实例 id 稳定且与显示名称无关。platform-key 为规范化 OS/CPU 标识，必要时加入 ABI，不能复用不同平台的 natives 或 Java。

`meta` 是共享的已安装依赖，不是“清缓存”目标。实例的 mods 等可变文件保持独立；不要与共享文件做可写硬链接。游戏文件名由 Minecraft 决定，不用启动器重命名。

## 4. 写入、删除与恢复

- launcher.db 保留事务边界和现有小表写入策略。schema 迁移先做一致备份；未来 schema 拒绝写入。数据库损坏时隔离原件并展示恢复模式，不静默以空库覆盖原库；从 profile 发现的目录只作为候选，不能猜测丢失配置。
- 小 JSON 采用 schema 字段、同目录临时文件、刷新后原子替换；无法读取的新版本文件保持原样。JSONL 保留逐行容错并报告跳过数量；压缩采用原子替换。
- 下载先校验 size/hash，再提交到最终位置。原子重命名只用于同文件系统；跨卷先复制到目标目录临时文件并校验，最后提交。需要提交原子性的临时文件放目标旁，不依赖 cache 与 data 同卷。
- 安装/更新日志记载操作 id、目标实例、输入与校验信息、阶段及提交结果，不记录秘密。启动恢复必须对照当前文件，已完成步骤不能重复覆盖用户内容；可恢复下载验证分片与服务器身份。当前活动任务只在内存，尚不具备此能力。
- 单 data 根只允许一个写入进程；共享资源安装以资源键锁定。持锁的启动/安装不能被 prune 干扰。PID 文件仅作诊断，不代替系统锁。
- 删除实例先写 pending-delete 标志并移到同卷隔离区，再完成数据库删除与磁盘清理；失败可重试/恢复。清理共享资源以实例、快照恢复需求、运行会话和安装操作的引用为依据。默认只报告可回收项，实际删除作为后续 Activity/Repairs 操作。
- 清 cache 后启动器应可重建展示缓存并启动已完整安装的游戏；中断中的下载允许重新开始。绝不清世界、快照、已安装 Java 或 meta。用户显式管理快照，默认不自动删除。
- 建议初始保留策略：应用日志 14 天且总量 100 MiB；完成活动最多 5,000 条；HTTP/图片缓存 LRU 总量 512 MiB；失去操作引用的临时下载 7 天后清除。这些是待实现的策略默认值，不是当前行为。实例历史和游戏日志不套用应用日志策略。
- 凭据及日志按最小权限创建；不记录 token、完整认证参数或设备登录秘密。在线账户公开信息和凭据引用可存 settings，秘密留给 macOS Keychain / Windows Credential Manager / Linux Secret Service；不可用时提供本次会话登录。

## 5. 启动与备份

启动顺序：解析路径 → 获取写锁 → 检查布局/schema → 恢复未完成迁移/操作 → 打开 store → 发布库快照 → 后台扫描实例/运行时/完整性。扫描和磁盘 I/O 不在 GPUI 线程。扫描中的实例有明确 loading 状态；失败信息交给现有 Needs Attention/Diagnostics。

备份分两档：个人数据备份含 SQLite 一致快照、config、profiles（含世界/快照），另可包含活动历史；完整离线备份再加入 meta 和 runtimes。cache、锁、临时工作区和凭据不导出。游戏运行时不能保证世界的一致备份，应先停止游戏或使用已有一致快照。数据库不能在写入时仅复制主文件，须使用 SQLite 备份能力或停写并处理 journal/WAL。

## 6. 与当前实现的差距及迁移

| 当前 | 目标 | 实施顺序 |
|---|---|---|
| app::backend::data_root 返回单根；Windows APPDATA | 平台解析返回 data/config/state/cache；Windows Local | P1 |
| Layout 仅包含主要游戏路径，部分模块直接 join | 所有应用自有路径统一通过 Layout；core 不读取平台环境 | P1 |
| 根目录 settings.json / activity.jsonl / downloads | config/settings、state/activity、cache/downloads | P2 |
| natives/<release>；Java 根路径由 locator 管理 | natives 与 managed runtimes 按平台隔离 | P2 |
| 内存任务板与完成日志 | 可恢复操作日志、单写者与资源锁 | P3 |
| 损坏 DB 隔离后启动空库 | 保留数据并进入恢复模式 | P3 |
| 离线账户在 settings | 保持兼容；在线秘密使用系统库 | 在线认证独立阶段 |

P1 验收：模拟三平台路径和覆盖优先级，覆盖缺失 HOME、相对 XDG、非 ASCII 路径；布局全部路径受控；纯 core 测试不依赖 GPUI。

P2 验收：保留现有 profiles/meta 和 release id；检测 Windows 旧 %APPDATA%/LumilioCL 与新根。两根均有库时不自动合并。迁移必须独占、记录阶段、复制校验后再切换，成功前保留源数据；磁盘不足/中断/跨卷后可重新进入。同根旧文件只有在目标缺失时导入，发生冲突报告并保留双方。存储布局版本与 DB schema 分开维护；新程序成功打开并校验后才提交布局标记。

P3 验收：注入提交前后崩溃、重复启动、失败删除、cache 全清、DB 损坏等场景，证明不覆盖世界、不重复副作用、完整安装仍可离线启动。测试按仓库 write-a-test 工作流添加。

无需导入或修改 Modrinth 的实际目录；截图只用来表达共享资源与实例分层的需求。自定义实例数据根、远程同步和自动共享资源回收不在首轮实施范围。
