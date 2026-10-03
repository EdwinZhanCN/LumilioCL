<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 设置 · 已实现的用户路径

表里每一行都有对应的实现；没做的、范围外的见 [../README.md](../README.md)。

| 操作 | 层 / 组件 | 结果与反馈 | 编号 | 备注 | 实现 |
|---|---|---|---|---|---|
| 外观 | 通用 · 分段：跟随系统 / 浅色 / 深色 | 立即生效并保存 | L-SET-01 |  | `lumilio-ui/src/pages/settings.rs` |
| 进入游戏后 | 通用 · 分段：保持 / 隐藏启动器 | 没有“关闭启动器”：启动器要守着游戏记录会话与游玩时间，关掉它游戏也会结束 | L-SET-01 |  | `lumilio-ui/src/pages/settings.rs` |
| 游戏退出后回到前台 | 通用 · 开关（默认开） | 游戏结束时把启动器带回最前面 | L-SET-01 |  | `lumilio-ui/src/pages/settings.rs` |
| 减少动效 | 通用 · 分段：跟随系统 / 减少 / 完整 | 立即生效 | L-SET-01 |  | `lumilio-ui/src/pages/settings.rs` |
| 最小 / 最大内存 | 游戏默认 · 值 + [编辑] 弹窗 | 弹窗说明本机内存与推荐值；恢复默认 = 推荐值 | L-SET-01 |  | `lumilio-ui/src/pages/settings.rs` |
| 窗口大小、全屏 | 游戏默认 · 值 + [编辑] 弹窗 | 宽高一起填；全屏 关 / 开 / 不设置 | L-SET-01 |  | `lumilio-ui/src/pages/settings.rs` |
| Java 参数 | 游戏默认 · 值 + [编辑] 弹窗 | 每行一项；游戏自己设置了参数时以游戏的为准 | L-SET-01 |  | `lumilio-ui/src/pages/settings.rs` |
| 游戏参数 | 游戏默认 · 值 + [编辑] 弹窗 | 每行一项 | L-SET-01 |  | `lumilio-ui/src/pages/settings.rs` |
| 环境变量 | 游戏默认 · 值 + [编辑] 弹窗 | 每行 名称=值 | L-SET-01 |  | `lumilio-ui/src/pages/settings.rs` |
| 启动前 / 包装 / 退出后命令 | 游戏默认 · 值 + [编辑] 弹窗 | 以当前用户权限运行；启动前命令失败取消启动，退出后命令失败只记录 | L-SET-01 |  | `lumilio-ui/src/pages/settings.rs` |
| Java：停用 / 启用 | Java · 列表行开关 | 停用不删除文件，之后不会被自动选中 | L-RUN-01 |  | `lumilio-ui/src/pages/settings.rs` |
| Java：在访达中显示 | Java · 列表行 ⋯ 菜单 | 打开并选中它所在位置 | L-RUN-01 |  | `lumilio-ui/src/pages/settings.rs` |
| Java：额外搜索目录 | Java · 值 + [编辑] 弹窗 | 每行一个文件夹，检测时也会找这些位置 | L-RUN-01 |  | `lumilio-ui/src/pages/settings.rs` |
| Java：检测与列表 | Java · 已发现的 Java 列表 | 列出检测到的 Java（版本、发行方、架构、路径）；没有时提示添加 | L-RUN-01 |  | `lumilio-ui/src/pages/settings.rs` |
| Java：重新检测 | Java · 按键「重新检测」 | 重新读取设置并检测 | L-RUN-01 |  | `lumilio-ui/src/pages/settings.rs` |
| Java：下载推荐的 Java | Java · 按键「下载推荐的 Java」 | 读 Mojang 的运行时索引，装好后出现在列表里；进度在动态 | H-SET-03 | ADR 0014 | `lumilio-ui/src/pages/settings.rs` |
| Java：添加 | Java · 按键「添加 Java…」→ 选 java 程序或 JDK 文件夹 | 加入列表 | L-RUN-01 |  | `lumilio-ui/src/pages/settings.rs` |
| 优先使用镜像 | 下载与存储 · 开关 | 先试镜像地址，不通再回到官方地址 | L-SET-01 |  | `lumilio-ui/src/pages/settings.rs` |
| 镜像规则 | 下载与存储 · 值 + [编辑] 弹窗 | 每行 官方前缀 => 镜像前缀 | L-SET-01 |  | `lumilio-ui/src/pages/settings.rs` |
| 同时下载数 | 下载与存储 · 值 + [编辑] 弹窗 | 自动 / 1–32 | L-SET-01 |  | `lumilio-ui/src/pages/settings.rs` |
| 数据目录 | 下载与存储 · 路径 + [在访达中显示] | 不可改（App State） | L-SET-01 |  | `lumilio-ui/src/pages/settings.rs` |
| 存储占用 | 下载与存储 · 游戏 / 共享资源 / Java / 缓存 条形图 | 后台计算后显示 | L-SET-01 |  | `lumilio-ui/src/pages/settings.rs` |
| 检查没用的游戏文件 | 下载与存储 · 按键「检查没用的游戏文件」 | 只报告能回收多少，确认后才删；游戏在安装/更新时拒绝 | — | ADR 0017 | `lumilio-ui/src/pages/settings.rs` |
| 清理缓存 | 下载与存储 · 按键「清理缓存」 | 缓存 = 已解压的 natives 与 cache/，下次启动会重建 | L-SET-01 |  | `lumilio-ui/src/pages/settings.rs` |
| 版本 | 关于 · 值 | 显示 LumilioCL 版本号 | L-SET-01 |  | `lumilio-ui/src/pages/settings.rs` |
| 启动器日志 | 关于 · 按键 | 在访达中显示日志目录 | L-SET-01 |  | `lumilio-ui/src/pages/settings.rs` |
| 导出诊断包 | 关于 · 按键 | 打包版本、设置摘要、Java 列表和各游戏最近日志；玩家名、UUID、路径脱敏 | L-SET-01 |  | `lumilio-ui/src/pages/settings.rs` |
| 开源许可 | 关于 · 值 | 显示 AGPL-3.0 | L-SET-01 |  | `lumilio-ui/src/pages/settings.rs` |
