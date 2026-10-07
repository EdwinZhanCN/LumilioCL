<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 设置 · 已实现的用户路径

表里每一行都有对应的实现；没有的就是没做。

| 操作 | 层 / 组件 | 结果与反馈 | 备注 | 实现 |
|---|---|---|---|---|
| 版本 | 关于 · 值 | 显示 LumilioCL 版本号 |  | `lumilio-ui/src/pages/settings/about.rs` |
| 启动器日志 | 关于 · 按键 | 在访达中显示日志目录 |  | `lumilio-ui/src/pages/settings/about.rs` |
| 导出诊断包 | 关于 · 按键 | 打包版本、设置摘要、Java 列表和各游戏最近日志；玩家名、UUID、路径脱敏 |  | `lumilio-ui/src/pages/settings/about.rs` |
| 开源许可 | 关于 · 值 | 显示 AGPL-3.0-only |  | `lumilio-ui/src/pages/settings/about.rs` |
| 最小 / 最大内存 | 游戏默认 · 值 + [编辑] 弹窗 | 弹窗说明本机内存与推荐值；恢复默认 = 推荐值 |  | `lumilio-ui/src/pages/settings/game_defaults.rs` |
| 窗口大小、全屏 | 游戏默认 · 值 + [编辑] 弹窗 | 宽高一起填；全屏 关 / 开 / 不设置 |  | `lumilio-ui/src/pages/settings/game_defaults.rs` |
| Java 参数 | 游戏默认 · 值 + [编辑] 弹窗 | 每行一项；游戏自己设置了参数时以游戏的为准 |  | `lumilio-ui/src/pages/settings/game_defaults.rs` |
| 游戏参数 | 游戏默认 · 值 + [编辑] 弹窗 | 每行一项 |  | `lumilio-ui/src/pages/settings/game_defaults.rs` |
| 环境变量 | 游戏默认 · 值 + [编辑] 弹窗 | 每行 名称=值 |  | `lumilio-ui/src/pages/settings/game_defaults.rs` |
| 启动前 / 包装 / 退出后命令 | 游戏默认 · 值 + [编辑] 弹窗 | 以当前用户权限运行；启动前命令失败取消启动，退出后命令失败只记录 |  | `lumilio-ui/src/pages/settings/game_defaults.rs` |
| 外观 | 通用 · 分段：跟随系统 / 浅色 / 深色 | 立即生效并保存 |  | `lumilio-ui/src/pages/settings/general.rs` |
| 进入游戏后 | 通用 · 分段：保持 / 隐藏启动器 | 没有“关闭启动器”：启动器要守着游戏记录会话与游玩时间，关掉它游戏也会结束 |  | `lumilio-ui/src/pages/settings/general.rs` |
| 游戏退出后回到前台 | 通用 · 开关（默认开） | 游戏结束时把启动器带回最前面 |  | `lumilio-ui/src/pages/settings/general.rs` |
| 减少动效 | 通用 · 分段：跟随系统 / 减少 / 完整 | 立即生效 |  | `lumilio-ui/src/pages/settings/general.rs` |
| Java：停用 / 启用 | Java · 列表行开关 | 停用不删除文件，之后不会被自动选中 |  | `lumilio-ui/src/pages/settings/java.rs` |
| Java：在访达中显示 | Java · 列表行 ⋯ 菜单 | 打开并选中它所在位置 |  | `lumilio-ui/src/pages/settings/java.rs` |
| Java：额外搜索目录 | Java · 值 + [编辑] 弹窗 | 每行一个文件夹，检测时也会找这些位置 |  | `lumilio-ui/src/pages/settings/java.rs` |
| Java：检测与列表 | Java · 已发现的 Java 列表 | 列出检测到的 Java（版本、发行方、架构、路径）；没有时提示添加 |  | `lumilio-ui/src/pages/settings/java.rs` |
| Java：重新检测 | Java · 按键「重新检测」 | 重新读取设置并检测 |  | `lumilio-ui/src/pages/settings/java.rs` |
| Java：下载推荐的 Java | Java · 按键「下载推荐的 Java」 | 读 Mojang 的运行时索引，装好后出现在列表里；进度在动态 | ADR 0014 | `lumilio-ui/src/pages/settings/java.rs` |
| Java：添加 | Java · 按键「添加 Java…」→ 选 java 程序或 JDK 文件夹 | 加入列表 |  | `lumilio-ui/src/pages/settings/java.rs` |
| 打开插件 | L3 设置 tab「插件」 | 显示核心插件、权限、运行状态和各插件的声明式设置；没有插件时显示空列表 |  | `lumilio-ui/src/pages/settings/mod.rs` |
| 选择插件 | 插件 · 左侧列表 | 右侧显示所选插件的状态、权限和设置 |  | `lumilio-ui/src/pages/settings/plugins.rs` |
| 插件：启用 / 停用 | 插件详情 · 设置行开关 | 保存开关；停用后贡献消失，运行失败的插件重启后恢复 |  | `lumilio-ui/src/pages/settings/plugins.rs` |
| 插件：查看权限 | 插件详情 · 权限行 | 逐条显示可读取的游戏文件夹、可访问的主机和其他能力 |  | `lumilio-ui/src/pages/settings/plugins.rs` |
| 插件：查看失败原因 | 插件详情 · [技术详情] | 显示本次调用失败原因；失败状态不会写入设置 |  | `lumilio-ui/src/pages/settings/plugins.rs` |
| 插件：更改设置 | 插件详情 · 声明式设置行 | 开关和选项立即保存；文字与数字在弹窗中校验并保存，失败保留草稿 |  | `lumilio-ui/src/pages/settings/plugins.rs` |
| 插件：恢复默认 | 插件详情 · [恢复默认] | 清除启用状态和设置覆盖，使用清单默认值；本次运行的失败状态保留 |  | `lumilio-ui/src/pages/settings/plugins.rs` |
| 添加 BMCLAPI 镜像 | 下载与存储 · 预设行 [添加] | 添加游戏资源、加载器与 authlib-injector 镜像；保留已有规则和优先顺序，完整添加后禁用按钮 |  | `lumilio-ui/src/pages/settings/storage.rs` |
| 添加 MCIM 镜像 | 下载与存储 · 预设行 [添加] | 添加 Modrinth / CurseForge 镜像；保留已有规则和优先顺序，完整添加后禁用按钮 |  | `lumilio-ui/src/pages/settings/storage.rs` |
| 添加腾讯 Maven 镜像 | 下载与存储 · 预设行 [添加] | 添加 Maven Central 镜像；保留已有规则和优先顺序，完整添加后禁用按钮 |  | `lumilio-ui/src/pages/settings/storage.rs` |
| 下载源 | 下载与存储 · 分段：仅官方 / 官方优先 / 镜像优先 | 立即保存；仅官方不访问镜像，其他模式按顺序回退，MCIM 始终位于官方之后 |  | `lumilio-ui/src/pages/settings/storage.rs` |
| 镜像规则 | 下载与存储 · 值 + [编辑] 弹窗 | 每行 官方前缀 => 镜像前缀 |  | `lumilio-ui/src/pages/settings/storage.rs` |
| 同时下载数 | 下载与存储 · 值 + [编辑] 弹窗 | 自动 / 1–32 |  | `lumilio-ui/src/pages/settings/storage.rs` |
| 数据目录 | 下载与存储 · 路径 + [在访达中显示] | 不可改（App State） |  | `lumilio-ui/src/pages/settings/storage.rs` |
| 存储占用 | 下载与存储 · 游戏 / 共享资源 / Java / 缓存 条形图 | 后台计算后显示 |  | `lumilio-ui/src/pages/settings/storage.rs` |
| 检查没用的游戏文件 | 下载与存储 · 按键「检查没用的游戏文件」 | 只报告能回收多少，确认后才删；游戏在安装/更新时拒绝 | ADR 0017 | `lumilio-ui/src/pages/settings/storage.rs` |
| 清理缓存 | 下载与存储 · 按键「清理缓存」 | 缓存 = 已解压的 natives 与 cache/，下次启动会重建 |  | `lumilio-ui/src/pages/settings/storage.rs` |
