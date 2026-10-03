# HMCL 用户流程对照

[返回入口](README.md) · [实例生命周期](instance-lifecycle.md) · [游玩与内容](play-and-content.md) · [状态与恢复](state-and-recovery.md)

## 基线、审计范围和证据等级

- 本地只读仓库：`3rd-party/HMCL`；HEAD：`587e92543a8926861f99fe11629d23dd94d090d8`，2026-08-25。
- 2026-09-30 核对的参考工作树干净。证据来自本地精确映射的入口、菜单、拖拽、向导及关键处理；未运行 HMCL，不声称跨平台或在线端到端验证通过。
- 「可达」表示从主导航或已可达页面有事件处理路径，不等于所有构建、平台、游戏版本、账户资格都可用。表内保留已观察到的限制。
- 合并同目标的按钮/右键/拖拽入口；浏览、取消选择等重复组合不单列。覆盖桌面 UI 流程家族，不声称覆盖未审计的命令行/文件关联启动，也不包含 Minecraft 内部菜单流程。
- 「内」表示可映射到现有 ARCH；「目标」仍需计划落地；「待」表示入口、协议或范围尚未决定；「外」不纳入当前产品。它们均不是实现状态。
- 所有证据由 [architecture](../architecture.md#user-workflow-reference-mapping) 的 FLOW-REF 表授权；本表引用函数/类仅为定位，不允许搬用到 crates。

## H-NAV — 进入与选择

| 编号 / 用户目标 | 入口与主流程 | 分支、限制或结果 | LumilioCL 归属 | 本地证据 |
|---|---|---|---|---|
| <a id="h-nav-01"></a>H-NAV-01 继续已有游戏 | 打开首页 → 恢复当前目录/实例选择 → 选择账户 → 启动 | 启动入口会补充账户选择；选择不是游戏运行证明 | 内：L-LIB-01、L-PLAY-01 | [R001](#flow-ref-001), [R002](#flow-ref-002), [R008](#flow-ref-008) |
| <a id="h-nav-02"></a>H-NAV-02 空库一键游玩 | 首页无实例 → 点击启动 → 自动安装最新受支持正式版 → 选中并启动 | 列表获取、安装可失败/取消；不是首次运行向导 | 内：L-LIB-02、L-PLAY-01 | [R002](#flow-ref-002) |
| <a id="h-nav-03"></a>H-NAV-03 切换当前实例 | 首页菜单、右键快捷列表或滚轮 → 选择实例 | 改变之后的启动目标，不应改变已经发起的操作 | 内：L-LIB-01 | [R001](#flow-ref-001), [R006](#flow-ref-006) |
| <a id="h-nav-04"></a>H-NAV-04 查找和刷新实例 | 实例列表 → 搜索或刷新 → 选择、管理或启动 | 空列表与加载失败有不同去向 | 内：L-LIB-01 | [R003](#flow-ref-003), [R004](#flow-ref-004) |
| <a id="h-nav-05"></a>H-NAV-05 登记游戏目录 | 实例列表 → 添加目录 → 输入名称/路径及相对路径选项 → 保存 | 名称/路径校验；只读存储有备份覆盖提示 | 待：外部数据根不在首轮 | [R003](#flow-ref-003), [R009](#flow-ref-009) |
| <a id="h-nav-06"></a>H-NAV-06 切换游戏目录 | 目录列表 → 点击目录 → 加载其仓库和实例 | 目录与实例是两层选择 | 待：外部数据根不在首轮 | [R011](#flow-ref-011), [R079](#flow-ref-079) |
| <a id="h-nav-07"></a>H-NAV-07 移除目录登记 | 目录行移除按钮 → 移除配置登记 → 必要时补回默认目录并切换选择 | 不执行游戏目录文件删除；未找到编辑已有目录的可达按钮 | 待：外部数据根不在首轮 | [R010](#flow-ref-010), [R011](#flow-ref-011), [R079](#flow-ref-079) |

## H-ACC — 账户和身份

| 编号 / 用户目标 | 入口与主流程 | 分支、限制或结果 | LumilioCL 归属 | 本地证据 |
|---|---|---|---|---|
| <a id="h-acc-01"></a>H-ACC-01 Microsoft 登录 | 账户管理 → 浏览器授权或设备码/二维码路径 → 登录完成 | 客户端注册/构建可能限制入口；用户可取消 | 待：L-ACC-02 | [R032](#flow-ref-032), [R035](#flow-ref-035) |
| <a id="h-acc-02"></a>H-ACC-02 离线账户创建 | 账户管理 → 输入角色名 → 可设置 UUID → 添加 | restricted 状态会禁用入口；不是所有构建无条件可用 | 内：L-ACC-01 | [R032](#flow-ref-032), [R034](#flow-ref-034) |
| <a id="h-acc-03"></a>H-ACC-03 登记第三方认证服务 | 账户管理 → 添加服务器 → URL → 获取信息 → 确认 | 无效地址/网络错误停留并可返回或取消 | 待：L-ACC-02 | [R032](#flow-ref-032), [R036](#flow-ref-036) |
| <a id="h-acc-04"></a>H-ACC-04 第三方账户登录 | 选择认证服务 → 输入凭据 → 认证 → 必要时选择角色 | 角色选择可取消；restricted 状态可能禁用入口 | 待：L-ACC-02 | [R032](#flow-ref-032), [R034](#flow-ref-034) |
| <a id="h-acc-05"></a>H-ACC-05 移除认证服务 | 认证服务行 → 移除 → 确认 | 只读配置先提示备份覆盖 | 待：L-ACC-02 | [R032](#flow-ref-032) |
| <a id="h-acc-06"></a>H-ACC-06 选择启动账户 | 账户列表、快捷菜单或账户滚轮 → 选择身份 | 与实例选择独立；不把角色 UUID 当实例 ID | 内：L-ACC-01 | [R001](#flow-ref-001), [R032](#flow-ref-032), [R033](#flow-ref-033) |
| <a id="h-acc-07"></a>H-ACC-07 刷新账户 | 账户行 → 刷新 → 更新账户信息 | 失败显示原因；部分 Microsoft 构建禁用刷新 | 待：L-ACC-02 | [R037](#flow-ref-037), [R038](#flow-ref-038) |
| <a id="h-acc-08"></a>H-ACC-08 移除账户 | 账户行 → 删除确认 → 移除 | 删除身份登记不删除实例/世界 | 内：L-ACC-01；在线待决策 | [R037](#flow-ref-037), [R038](#flow-ref-038) |
| <a id="h-acc-09"></a>H-ACC-09 复制角色 UUID | 账户行 → 复制 UUID → 剪贴板 | 只读操作 | 待：全局账户入口 | [R038](#flow-ref-038) |
| <a id="h-acc-10"></a>H-ACC-10 移动账户存储范围 | 账户行 → 用户级与便携存储之间移动 | 存储只读时提示备份覆盖；不搬入 LumilioCL 凭据设计 | 外：HMCL 存储特性 | [R038](#flow-ref-038) |
| <a id="h-acc-11"></a>H-ACC-11 配置离线皮肤 | 离线账户皮肤入口 → 默认/本地/皮肤服务 → 模型、皮肤和披风 → 确认 | 图片/服务可能失败；取消返回 | 待：皮肤能力未定 | [R037](#flow-ref-037), [R039](#flow-ref-039) |
| <a id="h-acc-12"></a>H-ACC-12 上传在线皮肤 | 支持上传的账户 → 选文件 → 上传 → 更新预览 | 是否允许取决于账户类型；错误不改变游戏文件 | 待：皮肤能力未定 | [R037](#flow-ref-037), [R038](#flow-ref-038) |

## H-INSTALL — 安装与导入

| 编号 / 用户目标 | 入口与主流程 | 分支、限制或结果 | LumilioCL 归属 | 本地证据 |
|---|---|---|---|---|
| <a id="h-install-01"></a>H-INSTALL-01 安装原版 | 下载 → 版本列表 → 选择版本 → 名称 → 安装 | 正式版/快照等列表过滤；名称或目标冲突拒绝 | 内：L-LIB-02 | [R015](#flow-ref-015), [R016](#flow-ref-016), [R017](#flow-ref-017) |
| <a id="h-install-02"></a>H-INSTALL-02 安装加载器组合 | 选择游戏版本 → 选择兼容组件及版本 → 安装 | 组件选择受游戏版本和相互兼容约束 | 内：L-LIB-02，加载器按当前能力 | [R017](#flow-ref-017), [R018](#flow-ref-018), [R019](#flow-ref-019) |
| <a id="h-install-03"></a>H-INSTALL-03 在线整合包安装 | 搜索整合包 → 项目/发布 → 下载归档 → 安装向导 | 创建实例，不作为普通内容文件放置 | 内：L-LIB-03 | [R008](#flow-ref-008), [R015](#flow-ref-015), [R021](#flow-ref-021), [R041](#flow-ref-041) |
| <a id="h-install-04"></a>H-INSTALL-04 本地整合包导入 | 列表/下载/首页导入或拖入归档 → 识别格式/编码 → 名称 → 安装 | ZIP/mrpack；识别失败或未知包进入不同处理 | 内：L-LIB-03；首轮 mrpack | [R001](#flow-ref-001), [R003](#flow-ref-003), [R021](#flow-ref-021), [R022](#flow-ref-022), [R023](#flow-ref-023) |
| <a id="h-install-05"></a>H-INSTALL-05 URL 归档导入 | 导入向导 → 输入 URL → 下载临时归档 → 解析安装 | 非法地址/下载失败返回提示；可取消 | 待：L-LIB-03 的来源扩展 | [R022](#flow-ref-022) |
| <a id="h-install-06"></a>H-INSTALL-06 服务器更新包安装 | 导入向导 → server manifest URL → 元数据 → 安装 | 专用服务器格式；不等同任意 JSON | 待：整合包协议范围 | [R022](#flow-ref-022), [R024](#flow-ref-024), [R031](#flow-ref-031) |
| <a id="h-install-07"></a>H-INSTALL-07 手工打包目录导入 | 本地包不能按标准识别 → 用户选择手工整合包路径 → 导入 | 依赖格式/目录识别；不能宣传支持所有 ZIP | 待：导入格式扩展 | [R023](#flow-ref-023), [R031](#flow-ref-031) |
| <a id="h-install-08"></a>H-INSTALL-08 版本 JSON 安装 | 首页拖入版本 JSON → 校验 → 新实例名称 → 下载/草稿提交 | 首页入口已核对；列表拖拽过滤存在差异，不保证所有 JSON 在列表可达 | 待：自定义 manifest 导入 | [R001](#flow-ref-001), [R003](#flow-ref-003), [R008](#flow-ref-008) |
| <a id="h-install-09"></a>H-INSTALL-09 安装结果处理 | 向导 → 任务进度 → 成功提示或按异常显示错误 → 结束 | 取消与失败分开；部分整合包异常有专门提示，不泛化为全部成功 | 内：L-OPS-01 | [R020](#flow-ref-020), [R021](#flow-ref-021), [R073](#flow-ref-073), [R074](#flow-ref-074) |

## H-INSTANCE — 实例生命周期

| 编号 / 用户目标 | 入口与主流程 | 分支、限制或结果 | LumilioCL 归属 | 本地证据 |
|---|---|---|---|---|
| <a id="h-instance-01"></a>H-INSTANCE-01 进入实例管理 | 实例行管理按钮/右键 → 游戏设置、组件、模组、资源包、世界、原理图 | 各子页依赖选中的实例和工作目录 | 内：L-LIB-01，原理图外 | [R004](#flow-ref-004), [R007](#flow-ref-007) |
| <a id="h-instance-02"></a>H-INSTANCE-02 重命名 | 管理菜单 → 新名称 → 合法性/冲突校验 → 修改并选中 | HMCL 修改实例标识；LumilioCL 显示名与稳定 ID 分开 | 内：L-LIB-04 | [R004](#flow-ref-004), [R008](#flow-ref-008) |
| <a id="h-instance-03"></a>H-INSTANCE-03 复制 | 管理菜单 → 新名称 → 是否复制存档 → 复制并发布 | 目标存在拒绝；草稿发布后才出现副本 | 内：L-LIB-05（目标） | [R004](#flow-ref-004), [R008](#flow-ref-008), [R080](#flow-ref-080) |
| <a id="h-instance-04"></a>H-INSTANCE-04 删除 | 管理菜单 → 展示隔离/非隔离相关删除提示 → 确认 → 删除 | 具体磁盘清理委托仓库；不能推导所有模式相同 | 内：L-LIB-06 | [R004](#flow-ref-004), [R007](#flow-ref-007), [R008](#flow-ref-008) |
| <a id="h-instance-05"></a>H-INSTANCE-05 实例图标 | 游戏设置 → 图标选择 → 应用或移除 | 图片处理有自己的失败路径 | 待：Overview/Settings 内能力 | [R067](#flow-ref-067) |
| <a id="h-instance-06"></a>H-INSTANCE-06 调整组件 | 自动安装页 → 添加/替换/移除组件 → 执行更新 | 可选项与组件依赖限制；禁止把全部加载器当作已支持 | 内：L-LIB-07（目标） | [R007](#flow-ref-007), [R019](#flow-ref-019), [R020](#flow-ref-020), [R026](#flow-ref-026) |
| <a id="h-instance-07"></a>H-INSTANCE-07 离线组件安装 | 组件页 → 选择安装器文件 → 执行 → 刷新 | 执行过程使用不可取消任务入口 | 待：L-LIB-07 的来源扩展 | [R026](#flow-ref-026) |
| <a id="h-instance-08"></a>H-INSTANCE-08 更新整合包 | 可更新实例 → 更新入口 → 选择来源 → 校验类型 → 更新 | 不支持/格式不匹配/损坏分别报错 | 内：L-LIB-07（目标），格式待定 | [R004](#flow-ref-004), [R007](#flow-ref-007), [R008](#flow-ref-008), [R021](#flow-ref-021), [R031](#flow-ref-031) |
| <a id="h-instance-09"></a>H-INSTANCE-09 导出整合包 | 管理菜单 → 格式 → 元数据 → 包含文件 → 输出文件 | MCBBS/MultiMC/服务器/Modrinth；选项随格式变化 | 待：L-LIB-08 | [R027](#flow-ref-027), [R028](#flow-ref-028), [R029](#flow-ref-029), [R030](#flow-ref-030) |
| <a id="h-instance-10"></a>H-INSTANCE-10 打开实例文件目录 | 浏览菜单 → 游戏/mods/资源包/世界/光影/截图/config/logs/crash-reports 等 | 交给系统文件管理器；不自动编辑文件 | 内：L-DIAG-01 | [R007](#flow-ref-007), [R008](#flow-ref-008) |
| <a id="h-instance-11"></a>H-INSTANCE-11 手动维护依赖和日志 | 管理菜单 → 重下资源索引、清资产/库、清理日志等 | 部分步骤不可取消；共享资源与 cache 必须在 LumilioCL 分开 | 内：L-OPS-02；破坏性依赖清理不照搬 | [R007](#flow-ref-007), [R008](#flow-ref-008), [R080](#flow-ref-080) |

## H-PLAY — 启动与游戏会话

| 编号 / 用户目标 | 入口与主流程 | 分支、限制或结果 | LumilioCL 归属 | 本地证据 |
|---|---|---|---|---|
| <a id="h-play-01"></a>H-PLAY-01 常规启动 | 首页/实例行 → 确保账户 → Java 检查 → 依赖补齐 → 认证 → 创建进程 → 等待启动 | 错误停留在对应阶段；运行与完成下载不同 | 内：L-PLAY-01 | [R002](#flow-ref-002), [R004](#flow-ref-004), [R008](#flow-ref-008), [R012](#flow-ref-012) |
| <a id="h-play-02"></a>H-PLAY-02 没有选中账户 | 启动 → 创建/选择账户对话框 → 成功后继续 | 取消且仍无账户则不启动 | 内：L-ACC-01、L-PLAY-01 | [R008](#flow-ref-008) |
| <a id="h-play-03"></a>H-PLAY-03 修正运行环境 | 启动检查 → Java/架构/内存/组件兼容建议 → 接受、改设置或取消 | 区分建议与硬性不满足；不保证所有建议可忽略 | 内：L-RUN-01、L-SET-01 | [R012](#flow-ref-012) |
| <a id="h-play-04"></a>H-PLAY-04 认证恢复 | 认证过期 → 重新登录；网络认证失败 → 重试、允许时离线或取消 | 离线资格由账户行为决定，不是任意失败降级 | 待：L-ACC-02 | [R012](#flow-ref-012), [R035](#flow-ref-035) |
| <a id="h-play-05"></a>H-PLAY-05 取消启动 | 启动任务 → 取消 → 停止任务；进程已创建时相应取消动作终止进程 | 不能把关掉页面当取消 | 内：L-PLAY-01、L-OPS-01 | [R012](#flow-ref-012), [R073](#flow-ref-073) |
| <a id="h-play-06"></a>H-PLAY-06 测试启动 | 实例菜单 → 测试游戏 → 保持诊断观察 | 不是另建实例 | 内：L-DIAG-01 | [R007](#flow-ref-007), [R008](#flow-ref-008), [R012](#flow-ref-012) |
| <a id="h-play-07"></a>H-PLAY-07 快速进入目标 | 世界菜单或 Quick Play 设置 → 启动目标世界/服务器/Realm | 游戏版本决定可用性；这是游戏内入口，不是联机隧道 | 待：L-PLAY-02 | [R048](#flow-ref-048), [R049](#flow-ref-049), [R067](#flow-ref-067) |
| <a id="h-play-08"></a>H-PLAY-08 生成启动脚本 | 实例或世界菜单 → 输出路径 → 准备启动参数 → 生成脚本 | 仍有账户/依赖要求；脚本包含认证信息时需另定导出策略 | 待：脚本能力未定 | [R008](#flow-ref-008), [R012](#flow-ref-012), [R048](#flow-ref-048) |
| <a id="h-play-09"></a>H-PLAY-09 观察/终止游戏 | 日志窗口 → 观察、筛选、复制、清显示、导出、可用时 dump → 终止 | 清显示不等同删磁盘日志；dump 依赖环境 | 内：L-PLAY-01、L-DIAG-01；dump 待定 | [R013](#flow-ref-013) |
| <a id="h-play-10"></a>H-PLAY-10 退出与崩溃 | 游戏退出 → 根据可见性策略恢复/关闭；异常退出 → 原因/日志/导出/帮助 | 保持、隐藏、关闭是不同策略；崩溃分析是线索 | 内：L-PLAY-01、L-DIAG-01 | [R012](#flow-ref-012), [R014](#flow-ref-014), [R067](#flow-ref-067) |

## H-DISC — 发现与发布详情

| 编号 / 用户目标 | 入口与主流程 | 分支、限制或结果 | LumilioCL 归属 | 本地证据 |
|---|---|---|---|---|
| <a id="h-disc-01"></a>H-DISC-01 浏览搜索 | 下载入口 → 选择类型/来源 → 查询、游戏版本、分类、排序、分页 | 来源和类型决定可用过滤条件；网络失败可重试 | 内：L-DISC-01；世界下载待定 | [R015](#flow-ref-015), [R040](#flow-ref-040), [R042](#flow-ref-042) |
| <a id="h-disc-02"></a>H-DISC-02 查看项目 | 搜索结果 → 项目详情 → 描述/发布列表/外部链接 | 详情读取失败不触发安装 | 内：L-DISC-01 | [R040](#flow-ref-040), [R041](#flow-ref-041) |
| <a id="h-disc-03"></a>H-DISC-03 查看发布和依赖 | 选择发布 → 文件信息、依赖、更新日志、发布页面 | 依赖详情加载失败可重试；展示依赖不证明自动安装全部依赖 | 内：L-DISC-01；依赖策略待定 | [R041](#flow-ref-041) |
| <a id="h-disc-04"></a>H-DISC-04 安装到目标实例 | 内容发布 → 安装回调 → 绑定实例 → 执行下载 | 普通内容与整合包回调不同；无回调只提供另存为 | 内：L-CONT-01、L-LIB-03 | [R015](#flow-ref-015), [R041](#flow-ref-041) |
| <a id="h-disc-05"></a>H-DISC-05 另存为文件 | 发布 → 另存为 → 指定位置 → 下载并校验 | 不自动写入实例；可取消 | 待：下载导出入口 | [R041](#flow-ref-041) |

## H-CONTENT — 本地内容与更新

| 编号 / 用户目标 | 入口与主流程 | 分支、限制或结果 | LumilioCL 归属 | 本地证据 |
|---|---|---|---|---|
| <a id="h-content-01"></a>H-CONTENT-01 扫描本地模组 | 实例 → 模组管理 → 刷新/搜索 → 查看列表 | 加载器支持决定管理状态；外部改动需刷新 | 内：L-CONT-02 | [R043](#flow-ref-043), [R044](#flow-ref-044) |
| <a id="h-content-02"></a>H-CONTENT-02 添加本地模组 | 添加或拖入多个文件 → 安装 → 分别报告成功/失败 | 批量结果不保证全部成功 | 内：L-CONT-01（本地接入目标） | [R043](#flow-ref-043) |
| <a id="h-content-03"></a>H-CONTENT-03 切换模组可用性 | 选择单个/多个模组 → 启用/禁用 | 禁用与删除不同 | 内：L-CONT-02 | [R044](#flow-ref-044) |
| <a id="h-content-04"></a>H-CONTENT-04 删除模组 | 选择模组 → 删除确认 → 删除 → 刷新 | 仅目标实例；用户可以取消选择/确认 | 内：L-CONT-02 | [R044](#flow-ref-044) |
| <a id="h-content-05"></a>H-CONTENT-05 查看模组信息 | 条目详情/定位 → 元数据、描述、相关链接或文件管理器 | 元数据缺失不等同文件无法使用 | 内：L-CONT-02、L-DIAG-01 | [R044](#flow-ref-044) |
| <a id="h-content-06"></a>H-CONTENT-06 检查并选择更新 | 全部或选中模组/资源包 → 检查 → 候选列表 → 选择、查看日志或导出清单 | 未知版本/无更新/请求失败分开；整合包内独立更新先警告 | 内：L-CONT-03 | [R043](#flow-ref-043), [R045](#flow-ref-045), [R046](#flow-ref-046), [R047](#flow-ref-047) |
| <a id="h-content-07"></a>H-CONTENT-07 执行内容更新 | 候选列表 → 执行选中项 → 逐项下载更新 → 结果 | 可部分成功，失败项单独列出；不是全批事务 | 内：L-CONT-03 | [R046](#flow-ref-046) |
| <a id="h-content-08"></a>H-CONTENT-08 模组版本回退 | 有旧版本记录的模组 → 选择旧版本 → 回退 → 刷新 | 没有旧版本不能凭空回退 | 待：L-CONT-03 的版本保留策略 | [R043](#flow-ref-043), [R044](#flow-ref-044) |
| <a id="h-content-09"></a>H-CONTENT-09 资源包导入与扫描 | 实例资源包 → 添加/拖入/下载 → 刷新/搜索 → 详情 | 格式/兼容问题有提示 | 内：L-CONT-01、L-CONT-02 | [R045](#flow-ref-045) |
| <a id="h-content-10"></a>H-CONTENT-10 资源包选择与删除 | 资源包 → 启用/禁用、批量操作、删除、定位 | 启用调用资源包管理器；不能等同模组文件禁用 | 内：文件管理；游戏选择语义待决策 | [R045](#flow-ref-045) |
| <a id="h-content-11"></a>H-CONTENT-11 光影下载与定位 | 下载页光影类型 → 发布 → 安装或另存为；实例菜单 → shaderpacks | 未发现独立光影管理页，不声称与模组管理功能对等 | 内：L-CONT-01；独立管理是 Lumilio 目标 | [R007](#flow-ref-007), [R015](#flow-ref-015), [R041](#flow-ref-041) |

## H-WORLD — 世界与数据包

| 编号 / 用户目标 | 入口与主流程 | 分支、限制或结果 | LumilioCL 归属 | 本地证据 |
|---|---|---|---|---|
| <a id="h-world-01"></a>H-WORLD-01 扫描世界 | 实例 → 世界 → 刷新 → 列表；可调整显示版本范围 | 受可识别世界、版本和锁状态限制 | 内：L-WORLD-01 | [R048](#flow-ref-048) |
| <a id="h-world-02"></a>H-WORLD-02 导入世界归档 | 添加文件或拖入 ZIP → 识别世界 → 安装 → 刷新 | 不是导入整合包；失败报错 | 待：L-WORLD-02 的导入能力 | [R048](#flow-ref-048) |
| <a id="h-world-03"></a>H-WORLD-03 在线下载世界 | 世界页下载 → 在线世界库 → 发布 → 下载 | 来源/回调决定另存为及安装路径，不保证统一自动安装 | 待：Discover 世界范围 | [R015](#flow-ref-015), [R048](#flow-ref-048) |
| <a id="h-world-04"></a>H-WORLD-04 进入世界管理 | 世界条目 → 世界信息/备份/支持时数据包 | 运行锁可使管理只读；加载失败关闭管理页 | 内：L-WORLD-01，编辑待定 | [R049](#flow-ref-049) |
| <a id="h-world-05"></a>H-WORLD-05 修改世界属性 | 世界信息 → 名称、图标、作弊、难度等可编辑属性 → 保存 | 字段由存档内容/版本决定；锁定状态限制写入 | 待：L-WORLD-02 的编辑能力 | [R050](#flow-ref-050) |
| <a id="h-world-06"></a>H-WORLD-06 查看/编辑玩家属性 | 世界信息 → 位置、出生点、游戏模式、生命、饥饿、经验等字段 | 并非任意存档都具备每个字段 | 待：L-WORLD-02 的编辑能力 | [R050](#flow-ref-050) |
| <a id="h-world-07"></a>H-WORLD-07 复制世界 | 世界菜单 → 新名称 → 复制 → 刷新 | 运行锁或同名冲突限制；失败报错 | 内：L-WORLD-02 | [R048](#flow-ref-048), [R055](#flow-ref-055) |
| <a id="h-world-08"></a>H-WORLD-08 删除世界 | 世界菜单 → 确认 → 删除 → 刷新/关闭页 | 运行锁限制；不删除整个实例 | 内：L-WORLD-02 | [R048](#flow-ref-048), [R055](#flow-ref-055) |
| <a id="h-world-09"></a>H-WORLD-09 导出世界 | 世界菜单 → 名称/输出文件 → 导出 | 运行锁限制；输出失败报错 | 待：L-WORLD-02 的导出能力 | [R048](#flow-ref-048), [R053](#flow-ref-053), [R054](#flow-ref-054), [R055](#flow-ref-055) |
| <a id="h-world-10"></a>H-WORLD-10 创建并管理世界备份 | 世界 → 备份 → 创建 → 列表 → 定位或删除 | 该页没有一键恢复按钮；归档再导入是另一流程 | 内：L-HIST-01；目标快照语义不同 | [R049](#flow-ref-049), [R051](#flow-ref-051), [R052](#flow-ref-052) |
| <a id="h-world-11"></a>H-WORLD-11 管理数据包 | 世界 → 数据包 → 添加/拖入/刷新/搜索 → 启用、禁用、删除、定位 | 只在受支持世界可达；不是实例级 mods | 待：Worlds 子能力，范围先定 | [R049](#flow-ref-049), [R056](#flow-ref-056), [R057](#flow-ref-057) |
| <a id="h-world-12"></a>H-WORLD-12 外部种子工具和文件定位 | 世界菜单 → 种子地图/结构工具或存档目录 | 外部浏览器/文件管理器；结构入口随版本变化 | 文件定位内：L-DIAG-01；种子工具外 | [R048](#flow-ref-048), [R049](#flow-ref-049) |

## H-SET — 设置、Java 与维护

| 编号 / 用户目标 | 入口与主流程 | 分支、限制或结果 | LumilioCL 归属 | 本地证据 |
|---|---|---|---|---|
| <a id="h-set-01"></a>H-SET-01 扫描 Java | 设置 → Java 管理 → 扫描/刷新 → 查看版本、架构、供应商 | 无可用运行时与扫描失败区分 | 内：L-RUN-01；全局 UI 待定 | [R058](#flow-ref-058), [R063](#flow-ref-063) |
| <a id="h-set-02"></a>H-SET-02 添加已装 Java | 添加/拖入可执行文件或 Java Home → 验证 → 登记 | 检测失败不当作可用运行时 | 内：L-RUN-01（接入目标） | [R063](#flow-ref-063) |
| <a id="h-set-03"></a>H-SET-03 安装 Java | 下载发行版或选择本地归档 → 信息与名称 → 安装 | 下载/解压/验证可失败；平台架构有约束 | 目标：L-RUN-01，托管管理待实现 | [R063](#flow-ref-063), [R064](#flow-ref-064), [R065](#flow-ref-065) |
| <a id="h-set-04"></a>H-SET-04 禁用/恢复外部 Java | 外部运行时 → 禁用 → 禁用列表 → 恢复；失效条目可移除 | 禁用不删除外部安装目录 | 目标：L-RUN-01 | [R063](#flow-ref-063), [R066](#flow-ref-066) |
| <a id="h-set-05"></a>H-SET-05 卸载托管 Java | 托管运行时 → 卸载确认 → 删除 | 与外部 Java 禁用不同 | 目标：L-RUN-01；引用规则待实现 | [R063](#flow-ref-063) |
| <a id="h-set-06"></a>H-SET-06 全局游戏默认 | 设置或实例列表全局入口 → 修改游戏默认 | 全局默认不覆盖实例显式设置 | 内：L-SET-01；全局入口待定 | [R003](#flow-ref-003), [R058](#flow-ref-058), [R067](#flow-ref-067) |
| <a id="h-set-07"></a>H-SET-07 游戏配置预设 | 创建/重命名/删除预设 → 实例选择预设 → 继承或覆盖项 | 预设体系是参考能力，不要求当前 Lumilio 实现 | 待：L-SET-01 扩展 | [R067](#flow-ref-067), [R068](#flow-ref-068) |
| <a id="h-set-08"></a>H-SET-08 实例基础设置 | 实例设置 → Java、内存、窗口、日志、隔离、运行目录、Quick Play 等 | 设置项可能继承；部分选项影响文件归属 | 内：L-SET-01；隔离切换不照搬 | [R067](#flow-ref-067) |
| <a id="h-set-09"></a>H-SET-09 实例高级设置 | JVM/游戏参数、环境、前后置命令、包装器、优先级、渲染、natives 等 | 平台和版本条件；高级选项不绕过 core 校验 | 内：Settings 五类；按计划接入 | [R067](#flow-ref-067) |
| <a id="h-set-10"></a>H-SET-10 下载和网络设置 | 设置 → 版本列表源、下载源、内容源、线程数、代理和缓存位置 | 设置变更不应重定向已发起任务目标 | 内：L-SET-01；更多入口待定 | [R060](#flow-ref-060) |
| <a id="h-set-11"></a>H-SET-11 清下载缓存 | 下载设置 → 清缓存 → 后续重新获取 | HMCL 缓存位置不是 Lumilio meta 删除授权 | 目标：L-OPS-02 | [R060](#flow-ref-060) |
| <a id="h-set-12"></a>H-SET-12 通用设置与更新 | 设置 → 语言/更新通道/预览及提示选项；检查更新 → 日志 → 接受 | 部分设置重启生效；更新执行不代表有回滚保证 | 待：应用维护范围 | [R059](#flow-ref-059), [R072](#flow-ref-072) |
| <a id="h-set-13"></a>H-SET-13 个性化 | 设置 → 明暗、主题色、背景、加载策略、透明度、字体和动画 | 预览与配置写入有自己的边界 | 外：不照搬 HMCL 主题体系 | [R061](#flow-ref-061) |
| <a id="h-set-14"></a>H-SET-14 主题包管理 | 导入/拖入 → 搜索 → 选择应用 → 定位/删除；可导出当前主题 | 应用步骤不可取消；外部格式专用 | 外：主题包无 ARCH 节点 | [R061](#flow-ref-061), [R062](#flow-ref-062) |
| <a id="h-set-15"></a>H-SET-15 帮助、反馈、关于及启动器日志 | 设置 → 文档/联系/项目信息；日志目录/日志导出 | 外部跳转不是业务任务成功；日志需脱敏 | 日志内：L-DIAG-01；全局入口待定 | [R059](#flow-ref-059), [R069](#flow-ref-069), [R070](#flow-ref-070), [R071](#flow-ref-071) |

## H-EXTRA — 联机与附加编辑器

| 编号 / 用户目标 | 入口与主流程 | 分支、限制或结果 | LumilioCL 归属 | 本地证据 |
|---|---|---|---|---|
| <a id="h-extra-01"></a>H-EXTRA-01 联机初始化 | 联机入口 → 协议 → 下载或导入组件 → 准备/启动 | 平台/架构/地区及组件条件限制 | 外：需范围 ADR | [R001](#flow-ref-001), [R075](#flow-ref-075), [R076](#flow-ref-076) |
| <a id="h-extra-02"></a>H-EXTRA-02 联机建房 | 选择房主 → 启动游戏或跳过 → 扫描局域网世界 → 建房 → 邀请码 | 扫描/连接可返回；不代替游戏开局域网 | 外：需范围 ADR | [R076](#flow-ref-076) |
| <a id="h-extra-03"></a>H-EXTRA-03 联机加入 | 输入邀请码 → 校验 → 连接 → 获取游戏连接指引 | 无效码/连接失败有独立状态 | 外：需范围 ADR | [R076](#flow-ref-076) |
| <a id="h-extra-04"></a>H-EXTRA-04 联机状态和恢复 | 查看玩家 → 返回退出房间；失败 → 重试、导出日志或反馈 | 临时网络会话不作为重启后已连接证明 | 外：需范围 ADR | [R076](#flow-ref-076) |
| <a id="h-extra-05"></a>H-EXTRA-05 原理图文件管理 | 实例 → 原理图 → 导入/拖入、目录浏览/创建、详情、定位/删除 | 这是文件管理与信息查看；不据此承诺 3D 查看器 | 外：需范围 ADR | [R007](#flow-ref-007), [R077](#flow-ref-077) |
| <a id="h-extra-06"></a>H-EXTRA-06 NBT 编辑 | 首页拖入受支持文件 → 加载树 → 修改 → 保存/取消 | 任意文件编辑不是当前 Diagnostics.Files 的隐式职责 | 外：需范围 ADR | [R001](#flow-ref-001), [R078](#flow-ref-078) |

## H-TASK — 横切交互

| 编号 / 用户目标 | 入口与主流程 | 分支、限制或结果 | LumilioCL 归属 | 本地证据 |
|---|---|---|---|---|
| <a id="h-task-01"></a>H-TASK-01 任务观察与取消 | 任务对话框 → 阶段、进度、速率 → 可取消时停止 | 不同操作可禁用取消；关闭页和操作终态不同 | 内：L-OPS-01 | [R073](#flow-ref-073), [R074](#flow-ref-074) |
| <a id="h-task-02"></a>H-TASK-02 向导返回与结束 | 配置页面返回/取消；执行后成功提示或错误提示 → 向导结束 | 重试应重新构建有效输入；不假定关闭后操作被回滚 | 内：L-OPS-01 | [R021](#flow-ref-021), [R027](#flow-ref-027), [R074](#flow-ref-074) |
| <a id="h-task-03"></a>H-TASK-03 只读设置处理 | 账户/目录/设置写入入口 → 提示备份与覆盖 → 用户决定 | 参考行为不允许 Lumilio 静默覆盖未来 schema | 目标：L-OPS-03 | [R009](#flow-ref-009), [R010](#flow-ref-010), [R032](#flow-ref-032), [R034](#flow-ref-034), [R067](#flow-ref-067) |
| <a id="h-task-04"></a>H-TASK-04 网络页面重试 | 搜索/项目/依赖加载失败 → 重试或返回 | 读取失败不创建实例或改变当前安装内容 | 内：L-DISC-01 | [R040](#flow-ref-040), [R041](#flow-ref-041) |

## 本地核对后的纠正与适配

1. 未从已审计导航找到编辑已有游戏目录的入口；`GameDirectoryPage` 接受已有对象并不证明用户可达。目录登记移除不删除游戏文件。
2. 版本 JSON 在首页有明确拖拽入口；实例列表拖拽的筛选与后续分支不一致，不能把两者写成等价能力。
3. 世界备份页面只有创建、浏览、定位、删除，没有直接恢复按钮。LumilioCL 的实例快照恢复是自有契约。
4. 模组回退依赖保留的旧版本；内容更新可部分成功。依赖详情展示不证明所有依赖自动安装。
5. 资源包选择与模组文件禁用不同；LumilioCL 当前统一文件可用性接口不能宣称控制 Minecraft 的资源包选择。
6. HMCL 名称/实例标识行为不照搬；LumilioCL 改显示名称不改变稳定实例 ID。
7. HMCL 的清库/清资源菜单不转化为 LumilioCL 清 cache 行为。meta 和 runtimes 是已安装依赖。
8. 在线账户离线模式资格、OAuth 构建条件和 restricted 提示均需保留条件，不把登录失败统一转换为离线身份。

以上限制属于证据边界；若后续找到新入口/处理，先补参考映射，再更新对应 H 编号，不从方法名猜测业务能力。

<a id="sources"></a>
## 精确本地源码索引

R 编号与 FLOW-REF 编号一一对应。文件内用下表的类/页面入口定位，再跟随表内 H 流程的操作处理；新增被调用文件必须先登记 architecture。

| 编号 | 本地文件与定位类 | 核对用途 |
|---|---|---|
| <a id="flow-ref-001"></a>R001 / FLOW-REF-001 | [ui/main/RootPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/RootPage.java) | 导航、实例与启动；RootPage 入口与处理 |
| <a id="flow-ref-002"></a>R002 / FLOW-REF-002 | [ui/main/MainPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/MainPage.java) | 导航、实例与启动；MainPage 入口与处理 |
| <a id="flow-ref-003"></a>R003 / FLOW-REF-003 | [ui/instances/GameListPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/GameListPage.java) | 导航、实例与启动；GameListPage 入口与处理 |
| <a id="flow-ref-004"></a>R004 / FLOW-REF-004 | [ui/instances/GameListCell](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/GameListCell.java) | 导航、实例与启动；GameListCell 入口与处理 |
| <a id="flow-ref-005"></a>R005 / FLOW-REF-005 | [ui/instances/GameListItem](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/GameListItem.java) | 导航、实例与启动；GameListItem 入口与处理 |
| <a id="flow-ref-006"></a>R006 / FLOW-REF-006 | [ui/instances/GameListPopupMenu](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/GameListPopupMenu.java) | 导航、实例与启动；GameListPopupMenu 入口与处理 |
| <a id="flow-ref-007"></a>R007 / FLOW-REF-007 | [ui/instances/GameInstancePage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/GameInstancePage.java) | 导航、实例与启动；GameInstancePage 入口与处理 |
| <a id="flow-ref-008"></a>R008 / FLOW-REF-008 | [ui/instances/Instances](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/Instances.java) | 导航、实例与启动；Instances 入口与处理 |
| <a id="flow-ref-009"></a>R009 / FLOW-REF-009 | [ui/directory/GameDirectoryPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/directory/GameDirectoryPage.java) | 导航、实例与启动；GameDirectoryPage 入口与处理 |
| <a id="flow-ref-010"></a>R010 / FLOW-REF-010 | [ui/directory/GameDirectoryListItem](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/directory/GameDirectoryListItem.java) | 导航、实例与启动；GameDirectoryListItem 入口与处理 |
| <a id="flow-ref-011"></a>R011 / FLOW-REF-011 | [ui/directory/GameDirectoryListItemSkin](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/directory/GameDirectoryListItemSkin.java) | 导航、实例与启动；GameDirectoryListItemSkin 入口与处理 |
| <a id="flow-ref-012"></a>R012 / FLOW-REF-012 | [game/LauncherHelper](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/game/LauncherHelper.java) | 导航、实例与启动；LauncherHelper 入口与处理 |
| <a id="flow-ref-013"></a>R013 / FLOW-REF-013 | [ui/LogWindow](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/LogWindow.java) | 导航、实例与启动；LogWindow 入口与处理 |
| <a id="flow-ref-014"></a>R014 / FLOW-REF-014 | [ui/GameCrashWindow](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/GameCrashWindow.java) | 导航、实例与启动；GameCrashWindow 入口与处理 |
| <a id="flow-ref-015"></a>R015 / FLOW-REF-015 | [ui/download/DownloadPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/DownloadPage.java) | 安装、导入与导出；DownloadPage 入口与处理 |
| <a id="flow-ref-016"></a>R016 / FLOW-REF-016 | [ui/download/VersionsPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/VersionsPage.java) | 安装、导入与导出；VersionsPage 入口与处理 |
| <a id="flow-ref-017"></a>R017 / FLOW-REF-017 | [ui/download/InstallersPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/InstallersPage.java) | 安装、导入与导出；InstallersPage 入口与处理 |
| <a id="flow-ref-018"></a>R018 / FLOW-REF-018 | [ui/download/AbstractInstallersPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/AbstractInstallersPage.java) | 安装、导入与导出；AbstractInstallersPage 入口与处理 |
| <a id="flow-ref-019"></a>R019 / FLOW-REF-019 | [ui/download/AdditionalInstallersPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/AdditionalInstallersPage.java) | 安装、导入与导出；AdditionalInstallersPage 入口与处理 |
| <a id="flow-ref-020"></a>R020 / FLOW-REF-020 | [ui/download/UpdateInstallerWizardProvider](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/UpdateInstallerWizardProvider.java) | 安装、导入与导出；UpdateInstallerWizardProvider 入口与处理 |
| <a id="flow-ref-021"></a>R021 / FLOW-REF-021 | [ui/download/ModpackInstallWizardProvider](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/ModpackInstallWizardProvider.java) | 安装、导入与导出；ModpackInstallWizardProvider 入口与处理 |
| <a id="flow-ref-022"></a>R022 / FLOW-REF-022 | [ui/download/ModpackSelectionPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/ModpackSelectionPage.java) | 安装、导入与导出；ModpackSelectionPage 入口与处理 |
| <a id="flow-ref-023"></a>R023 / FLOW-REF-023 | [ui/download/LocalModpackPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/LocalModpackPage.java) | 安装、导入与导出；LocalModpackPage 入口与处理 |
| <a id="flow-ref-024"></a>R024 / FLOW-REF-024 | [ui/download/RemoteModpackPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/RemoteModpackPage.java) | 安装、导入与导出；RemoteModpackPage 入口与处理 |
| <a id="flow-ref-025"></a>R025 / FLOW-REF-025 | [ui/download/ModpackPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/ModpackPage.java) | 安装、导入与导出；ModpackPage 入口与处理 |
| <a id="flow-ref-026"></a>R026 / FLOW-REF-026 | [ui/instances/InstallerListPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/InstallerListPage.java) | 安装、导入与导出；InstallerListPage 入口与处理 |
| <a id="flow-ref-027"></a>R027 / FLOW-REF-027 | [ui/export/ExportWizardProvider](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/export/ExportWizardProvider.java) | 安装、导入与导出；ExportWizardProvider 入口与处理 |
| <a id="flow-ref-028"></a>R028 / FLOW-REF-028 | [ui/export/ModpackTypeSelectionPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/export/ModpackTypeSelectionPage.java) | 安装、导入与导出；ModpackTypeSelectionPage 入口与处理 |
| <a id="flow-ref-029"></a>R029 / FLOW-REF-029 | [ui/export/ModpackInfoPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/export/ModpackInfoPage.java) | 安装、导入与导出；ModpackInfoPage 入口与处理 |
| <a id="flow-ref-030"></a>R030 / FLOW-REF-030 | [ui/export/ModpackFileSelectionPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/export/ModpackFileSelectionPage.java) | 安装、导入与导出；ModpackFileSelectionPage 入口与处理 |
| <a id="flow-ref-031"></a>R031 / FLOW-REF-031 | [game/ModpackHelper](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/game/ModpackHelper.java) | 安装、导入与导出；ModpackHelper 入口与处理 |
| <a id="flow-ref-032"></a>R032 / FLOW-REF-032 | [ui/account/AccountListPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/account/AccountListPage.java) | 账户；AccountListPage 入口与处理 |
| <a id="flow-ref-033"></a>R033 / FLOW-REF-033 | [ui/account/AccountListPopupMenu](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/account/AccountListPopupMenu.java) | 账户；AccountListPopupMenu 入口与处理 |
| <a id="flow-ref-034"></a>R034 / FLOW-REF-034 | [ui/account/CreateAccountPane](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/account/CreateAccountPane.java) | 账户；CreateAccountPane 入口与处理 |
| <a id="flow-ref-035"></a>R035 / FLOW-REF-035 | [ui/account/MicrosoftAccountLoginPane](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/account/MicrosoftAccountLoginPane.java) | 账户；MicrosoftAccountLoginPane 入口与处理 |
| <a id="flow-ref-036"></a>R036 / FLOW-REF-036 | [ui/account/AddAuthlibInjectorServerPane](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/account/AddAuthlibInjectorServerPane.java) | 账户；AddAuthlibInjectorServerPane 入口与处理 |
| <a id="flow-ref-037"></a>R037 / FLOW-REF-037 | [ui/account/AccountListItem](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/account/AccountListItem.java) | 账户；AccountListItem 入口与处理 |
| <a id="flow-ref-038"></a>R038 / FLOW-REF-038 | [ui/account/AccountListItemSkin](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/account/AccountListItemSkin.java) | 账户；AccountListItemSkin 入口与处理 |
| <a id="flow-ref-039"></a>R039 / FLOW-REF-039 | [ui/account/OfflineAccountSkinPane](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/account/OfflineAccountSkinPane.java) | 账户；OfflineAccountSkinPane 入口与处理 |
| <a id="flow-ref-040"></a>R040 / FLOW-REF-040 | [ui/instances/DownloadListPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/DownloadListPage.java) | 内容与世界；DownloadListPage 入口与处理 |
| <a id="flow-ref-041"></a>R041 / FLOW-REF-041 | [ui/instances/DownloadPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/DownloadPage.java) | 内容与世界；DownloadPage 入口与处理 |
| <a id="flow-ref-042"></a>R042 / FLOW-REF-042 | [ui/instances/HMCLLocalizedDownloadListPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/HMCLLocalizedDownloadListPage.java) | 内容与世界；HMCLLocalizedDownloadListPage 入口与处理 |
| <a id="flow-ref-043"></a>R043 / FLOW-REF-043 | [ui/instances/ModListPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/ModListPage.java) | 内容与世界；ModListPage 入口与处理 |
| <a id="flow-ref-044"></a>R044 / FLOW-REF-044 | [ui/instances/ModListPageSkin](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/ModListPageSkin.java) | 内容与世界；ModListPageSkin 入口与处理 |
| <a id="flow-ref-045"></a>R045 / FLOW-REF-045 | [ui/instances/ResourcePackListPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/ResourcePackListPage.java) | 内容与世界；ResourcePackListPage 入口与处理 |
| <a id="flow-ref-046"></a>R046 / FLOW-REF-046 | [ui/instances/AddonUpdatesPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/AddonUpdatesPage.java) | 内容与世界；AddonUpdatesPage 入口与处理 |
| <a id="flow-ref-047"></a>R047 / FLOW-REF-047 | [ui/instances/AddonCheckUpdatesTask](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/AddonCheckUpdatesTask.java) | 内容与世界；AddonCheckUpdatesTask 入口与处理 |
| <a id="flow-ref-048"></a>R048 / FLOW-REF-048 | [ui/instances/WorldListPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/WorldListPage.java) | 内容与世界；WorldListPage 入口与处理 |
| <a id="flow-ref-049"></a>R049 / FLOW-REF-049 | [ui/instances/WorldManagePage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/WorldManagePage.java) | 内容与世界；WorldManagePage 入口与处理 |
| <a id="flow-ref-050"></a>R050 / FLOW-REF-050 | [ui/instances/WorldInfoPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/WorldInfoPage.java) | 内容与世界；WorldInfoPage 入口与处理 |
| <a id="flow-ref-051"></a>R051 / FLOW-REF-051 | [ui/instances/WorldBackupsPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/WorldBackupsPage.java) | 内容与世界；WorldBackupsPage 入口与处理 |
| <a id="flow-ref-052"></a>R052 / FLOW-REF-052 | [ui/instances/WorldBackupTask](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/WorldBackupTask.java) | 内容与世界；WorldBackupTask 入口与处理 |
| <a id="flow-ref-053"></a>R053 / FLOW-REF-053 | [ui/instances/WorldExportPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/WorldExportPage.java) | 内容与世界；WorldExportPage 入口与处理 |
| <a id="flow-ref-054"></a>R054 / FLOW-REF-054 | [ui/instances/WorldExportPageSkin](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/WorldExportPageSkin.java) | 内容与世界；WorldExportPageSkin 入口与处理 |
| <a id="flow-ref-055"></a>R055 / FLOW-REF-055 | [ui/instances/WorldManageUIUtils](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/WorldManageUIUtils.java) | 内容与世界；WorldManageUIUtils 入口与处理 |
| <a id="flow-ref-056"></a>R056 / FLOW-REF-056 | [ui/instances/DataPackListPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/DataPackListPage.java) | 内容与世界；DataPackListPage 入口与处理 |
| <a id="flow-ref-057"></a>R057 / FLOW-REF-057 | [ui/instances/DataPackListPageSkin](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/DataPackListPageSkin.java) | 内容与世界；DataPackListPageSkin 入口与处理 |
| <a id="flow-ref-058"></a>R058 / FLOW-REF-058 | [ui/main/LauncherSettingsPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/LauncherSettingsPage.java) | 设置、Java 与应用维护；LauncherSettingsPage 入口与处理 |
| <a id="flow-ref-059"></a>R059 / FLOW-REF-059 | [ui/main/SettingsPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/SettingsPage.java) | 设置、Java 与应用维护；SettingsPage 入口与处理 |
| <a id="flow-ref-060"></a>R060 / FLOW-REF-060 | [ui/main/DownloadSettingsPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/DownloadSettingsPage.java) | 设置、Java 与应用维护；DownloadSettingsPage 入口与处理 |
| <a id="flow-ref-061"></a>R061 / FLOW-REF-061 | [ui/main/PersonalizationPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/PersonalizationPage.java) | 设置、Java 与应用维护；PersonalizationPage 入口与处理 |
| <a id="flow-ref-062"></a>R062 / FLOW-REF-062 | [ui/main/ThemePackManagementPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/ThemePackManagementPage.java) | 设置、Java 与应用维护；ThemePackManagementPage 入口与处理 |
| <a id="flow-ref-063"></a>R063 / FLOW-REF-063 | [ui/main/JavaManagementPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/JavaManagementPage.java) | 设置、Java 与应用维护；JavaManagementPage 入口与处理 |
| <a id="flow-ref-064"></a>R064 / FLOW-REF-064 | [ui/main/JavaInstallPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/JavaInstallPage.java) | 设置、Java 与应用维护；JavaInstallPage 入口与处理 |
| <a id="flow-ref-065"></a>R065 / FLOW-REF-065 | [ui/main/JavaDownloadDialog](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/JavaDownloadDialog.java) | 设置、Java 与应用维护；JavaDownloadDialog 入口与处理 |
| <a id="flow-ref-066"></a>R066 / FLOW-REF-066 | [ui/main/JavaRestorePage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/JavaRestorePage.java) | 设置、Java 与应用维护；JavaRestorePage 入口与处理 |
| <a id="flow-ref-067"></a>R067 / FLOW-REF-067 | [ui/game/GameSettingsPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/game/GameSettingsPage.java) | 设置、Java 与应用维护；GameSettingsPage 入口与处理 |
| <a id="flow-ref-068"></a>R068 / FLOW-REF-068 | [ui/game/PresetManagementPane](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/game/PresetManagementPane.java) | 设置、Java 与应用维护；PresetManagementPane 入口与处理 |
| <a id="flow-ref-069"></a>R069 / FLOW-REF-069 | [ui/main/HelpPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/HelpPage.java) | 设置、Java 与应用维护；HelpPage 入口与处理 |
| <a id="flow-ref-070"></a>R070 / FLOW-REF-070 | [ui/main/FeedbackPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/FeedbackPage.java) | 设置、Java 与应用维护；FeedbackPage 入口与处理 |
| <a id="flow-ref-071"></a>R071 / FLOW-REF-071 | [ui/main/AboutPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/AboutPage.java) | 设置、Java 与应用维护；AboutPage 入口与处理 |
| <a id="flow-ref-072"></a>R072 / FLOW-REF-072 | [ui/UpgradeDialog](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/UpgradeDialog.java) | 设置、Java 与应用维护；UpgradeDialog 入口与处理 |
| <a id="flow-ref-073"></a>R073 / FLOW-REF-073 | [ui/construct/TaskExecutorDialogPane](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/construct/TaskExecutorDialogPane.java) | 设置、Java 与应用维护；TaskExecutorDialogPane 入口与处理 |
| <a id="flow-ref-074"></a>R074 / FLOW-REF-074 | [ui/wizard/TaskExecutorDialogWizardDisplayer](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/wizard/TaskExecutorDialogWizardDisplayer.java) | 设置、Java 与应用维护；TaskExecutorDialogWizardDisplayer 入口与处理 |
| <a id="flow-ref-075"></a>R075 / FLOW-REF-075 | [ui/terracotta/TerracottaPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/terracotta/TerracottaPage.java) | 范围外候选核对；TerracottaPage 入口与处理 |
| <a id="flow-ref-076"></a>R076 / FLOW-REF-076 | [ui/terracotta/TerracottaControllerPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/terracotta/TerracottaControllerPage.java) | 范围外候选核对；TerracottaControllerPage 入口与处理 |
| <a id="flow-ref-077"></a>R077 / FLOW-REF-077 | [ui/instances/SchematicsPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/SchematicsPage.java) | 范围外候选核对；SchematicsPage 入口与处理 |
| <a id="flow-ref-078"></a>R078 / FLOW-REF-078 | [ui/nbt/NBTEditorPage](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/nbt/NBTEditorPage.java) | 范围外候选核对；NBTEditorPage 入口与处理 |
| <a id="flow-ref-079"></a>R079 / FLOW-REF-079 | [setting/GameDirectoryManager](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/setting/GameDirectoryManager.java) | 目录登记、移除及选择；GameDirectoryManager 入口与处理 |
| <a id="flow-ref-080"></a>R080 / FLOW-REF-080 | [game/HMCLGameRepository](../../3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/game/HMCLGameRepository.java) | 实例复制和发布；HMCLGameRepository 入口与处理 |
