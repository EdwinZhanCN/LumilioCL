<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 设置 · 已实现的用户路径

表里每一行都有对应的实现；没有的就是没做。

| 操作 | 层 / 组件 | 结果与反馈 | 备注 | 实现 |
|---|---|---|---|---|
| 版本 | 关于 · 值 | 显示 LumilioCL 版本号 |  | `lumilio-ui/src/features/settings/about.rs` |
| 更新内容 | 关于 · 按键 | 打开这个版本的更新内容弹窗（新功能、改进、修复）；没有这个版本的说明时不显示按键 |  | `lumilio-ui/src/features/settings/about.rs` |
| 手动检查更新 | 关于 · 按键 | 检查正式版并显示进度、结果或失败原因 |  | `lumilio-ui/src/features/settings/about.rs` |
| 自动更新 | 关于 · 开关 | 开启时启动后与每小时静默检查并下载；关闭后只保留手动检查 |  | `lumilio-ui/src/features/settings/about.rs` |
| 启动器日志 | 关于 · 按键 | 在访达中显示启动器日志（警告与错误、启动与游戏结束，满 1 MB 换新文件并保留上一个） |  | `lumilio-ui/src/features/settings/about.rs` |
| 导出诊断包 | 关于 · 按键 | 打包版本、设置摘要、Java 列表、启动器日志和各游戏最近日志；玩家名、UUID、路径脱敏 |  | `lumilio-ui/src/features/settings/about.rs` |
| 免责声明 | 关于 · 按键 | 打开第一次打开时同意的免责声明，只读；「阅读 Minecraft EULA」在浏览器打开 EULA |  | `lumilio-ui/src/features/settings/about.rs` |
| 开源许可 | 关于 · 值 | 显示 AGPL-3.0-only |  | `lumilio-ui/src/features/settings/about.rs` |
| 明暗 | 外观 · 分段：跟随系统 / 浅色 / 深色 | 立即生效并保存 |  | `lumilio-ui/src/features/settings/appearance.rs` |
| 浅色主题 | 外观 · 值 + 更改 → 对话框里可搜索的主题列表 | 立即切换并保存；“恢复默认”回到内置的铝 |  | `lumilio-ui/src/features/settings/appearance.rs` |
| 深色主题 | 外观 · 值 + 更改 → 对话框里可搜索的主题列表 | 立即切换并保存；“恢复默认”回到内置的夜 |  | `lumilio-ui/src/features/settings/appearance.rs` |
| 界面字体 | 外观 · 值 + 更改 → 对话框里可搜索的已装字体 | 立即生效并保存；“恢复默认”回到 Space Grotesk |  | `lumilio-ui/src/features/settings/appearance.rs` |
| 等宽字体 | 外观 · 值 + 更改 → 对话框里可搜索的已装字体 | 版本号、数值和游戏日志跟着变；“恢复默认”回到 JetBrains Mono |  | `lumilio-ui/src/features/settings/appearance.rs` |
| 中文字体 | 外观 · 值 + 更改 → 对话框里可搜索的已装字体 | 汉字优先用它；没装就回退系统中文字体并在这里说明，不会用拉丁字体画汉字 |  | `lumilio-ui/src/features/settings/appearance.rs` |
| 界面缩放 | 外观 · 分段：85% / 100% / 115% / 130% | 文字和间距一起缩放，立即生效并保存；范围有上下限，防止破版 |  | `lumilio-ui/src/features/settings/appearance.rs` |
| 主页壁纸 | 外观 · 按键「选择图片…」→ 系统文件选择器 | 图片缩小后存进启动器文件夹，主页换成这张图，底部仍溶解进页面；不是图片就提示，当前壁纸不变 |  | `lumilio-ui/src/features/settings/appearance.rs` |
| 不用壁纸 | 外观 · 按键「不用壁纸」 | 主页不画世界，标题与「继续」直接放在页面上，半透明主题透出桌面；存下的图片一并删除 | translucent-themes T1 | `lumilio-ui/src/features/settings/appearance.rs` |
| 恢复默认壁纸 | 外观 · 按键 | 主页回到默认的像素世界，存下的图片一并删除 |  | `lumilio-ui/src/features/settings/appearance.rs` |
| 重新读取本地主题 | 外观 · 按键 | 重新扫描 themes 文件夹；能用的出现在上面的列表里，不能用的在下面说明原因 |  | `lumilio-ui/src/features/settings/appearance.rs` |
| 打开主题文件夹 | 外观 · 按键 | 在系统文件管理器里显示 themes 文件夹，没有就先建好 |  | `lumilio-ui/src/features/settings/appearance.rs` |
| 最小 / 最大内存 | 游戏默认 · 值 + [编辑] 弹窗 | 弹窗说明本机内存与推荐值；恢复默认 = 推荐值 |  | `lumilio-ui/src/features/settings/game_defaults.rs` |
| 窗口大小、全屏 | 游戏默认 · 值 + [编辑] 弹窗 | 宽高一起填；全屏 关 / 开 / 不设置 |  | `lumilio-ui/src/features/settings/game_defaults.rs` |
| Java 参数 | 游戏默认 · 值 + [编辑] 弹窗 | 每行一项；游戏自己设置了参数时以游戏的为准 |  | `lumilio-ui/src/features/settings/game_defaults.rs` |
| 游戏参数 | 游戏默认 · 值 + [编辑] 弹窗 | 每行一项 |  | `lumilio-ui/src/features/settings/game_defaults.rs` |
| 环境变量 | 游戏默认 · 值 + [编辑] 弹窗 | 每行 名称=值 |  | `lumilio-ui/src/features/settings/game_defaults.rs` |
| 启动前 / 包装 / 退出后命令 | 游戏默认 · 值 + [编辑] 弹窗 | 以当前用户权限运行；启动前命令失败取消启动，退出后命令失败只记录 |  | `lumilio-ui/src/features/settings/game_defaults.rs` |
| 语言 | 通用 · 分段：跟随系统 / 简体中文 / English | 立即切换并保存，组件自带的文字一起换；跟随系统时取系统语言里第一个中文或英文，都没有则用简体中文 |  | `lumilio-ui/src/features/settings/general.rs` |
| 开启翻译 | 通用 · 开关（默认关） | 开启后发现列表与详情页头自动显示中文简介（先 MCIM，再 launcher.lumilio.org），正文点击翻译；关闭立即恢复原文 |  | `lumilio-ui/src/features/settings/general.rs` |
| 翻译提供商 | 通用 · 下拉框 | 正文用的实时翻译；目前只有微软 Edge 并默认选中，MCIM 只供简介、不在此列 |  | `lumilio-ui/src/features/settings/general.rs` |
| 清除翻译缓存 | 通用 · 按钮 | 删除本地保存的译文，下次翻译重新请求；结果以提示告知 |  | `lumilio-ui/src/features/settings/general.rs` |
| 进入游戏后 | 通用 · 分段：保持 / 隐藏启动器 | 没有“关闭启动器”：启动器要守着游戏记录会话与游玩时间，关掉它游戏也会结束 |  | `lumilio-ui/src/features/settings/general.rs` |
| 游戏退出后回到前台 | 通用 · 开关（默认开） | 游戏结束时把启动器带回最前面 |  | `lumilio-ui/src/features/settings/general.rs` |
| 减少动效 | 通用 · 分段：跟随系统 / 减少 / 完整 | 立即生效 |  | `lumilio-ui/src/features/settings/general.rs` |
| Java：停用 / 启用 | Java · 列表行开关 | 停用不删除文件，之后不会被自动选中 |  | `lumilio-ui/src/features/settings/java.rs` |
| Java：在访达中显示 | Java · 列表行 ⋯ 菜单 | 打开并选中它所在位置 |  | `lumilio-ui/src/features/settings/java.rs` |
| Java：删除 | Java · 列表行 ⋯ 菜单「删除…」（只有启动器下载的 Java）→ 确认框 | 删除它的文件夹并重新检测；其它 Java 只能停用 |  | `lumilio-ui/src/features/settings/java.rs` |
| Java：额外搜索目录 | Java · 值 + [编辑] 弹窗 | 每行一个文件夹，检测时也会找这些位置 |  | `lumilio-ui/src/features/settings/java.rs` |
| Java：下载来源 | Java · 分段「Mojang / Oracle GraalVM」 | 下载与启动时自动下载都从这里取；GraalVM 只有 Java 21 和 25，其它版本仍从 Mojang 下载 |  | `lumilio-ui/src/features/settings/java.rs` |
| Java：启动时自动下载 | Java · 开关 | 开启时，启动或安装时没有合适的 Java 就先下载游戏需要的版本，进度在动态；关闭后提示去安装 |  | `lumilio-ui/src/features/settings/java.rs` |
| Java：下载 Java… | Java · 按键「下载 Java…」→ 弹窗选大版本 | 版本来自所选下载来源（Mojang 读运行时索引），默认 21；装好后出现在列表里，进度在动态 | ADR 0014 | `lumilio-ui/src/features/settings/java.rs` |
| Java：检测与列表 | Java · 已发现的 Java 列表 | 列出检测到的 Java（版本、发行方、架构、路径），包括官方启动器的运行时与 PATH 中的 java；没有时提示添加 |  | `lumilio-ui/src/features/settings/java.rs` |
| Java：重新检测 | Java · 按键「重新检测」 | 重新读取设置并检测 |  | `lumilio-ui/src/features/settings/java.rs` |
| Java：添加 | Java · 按键「添加 Java…」→ 选 java 程序或 JDK 文件夹 | 加入列表 |  | `lumilio-ui/src/features/settings/java.rs` |
| 打开插件 | L3 设置 tab「插件」 | 显示核心插件、权限、运行状态和各插件的声明式设置；没有插件时显示空列表 |  | `lumilio-ui/src/features/settings/page.rs` |
| 选择插件 | 插件 · 左侧列表 | 右侧显示所选插件的状态、权限和设置 |  | `lumilio-ui/src/features/settings/plugins.rs` |
| 插件：启用 / 停用 | 插件详情 · 设置行开关 | 保存开关；停用后贡献消失，运行失败的插件重启后恢复 |  | `lumilio-ui/src/features/settings/plugins.rs` |
| 插件：查看权限 | 插件详情 · 权限行 | 逐条显示可读取的游戏文件夹、可访问的主机和其他能力 |  | `lumilio-ui/src/features/settings/plugins.rs` |
| 插件：查看失败原因 | 插件详情 · [技术详情] | 显示本次调用失败原因；失败状态不会写入设置 |  | `lumilio-ui/src/features/settings/plugins.rs` |
| 插件：更改设置 | 插件详情 · 声明式设置行 | 开关和选项立即保存；文字与数字在弹窗中校验并保存，失败保留草稿 |  | `lumilio-ui/src/features/settings/plugins.rs` |
| 插件：恢复默认 | 插件详情 · [恢复默认] | 清除启用状态和设置覆盖，使用清单默认值；本次运行的失败状态保留 |  | `lumilio-ui/src/features/settings/plugins.rs` |
| 添加 BMCLAPI 镜像 | 下载与存储 · 预设行 [添加] | 添加游戏资源、加载器与 authlib-injector 镜像；保留已有规则和优先顺序，完整添加后禁用按钮 |  | `lumilio-ui/src/features/settings/storage.rs` |
| 添加 MCIM 镜像 | 下载与存储 · 预设行 [添加] | 添加 Modrinth / CurseForge 镜像；保留已有规则和优先顺序，完整添加后禁用按钮 |  | `lumilio-ui/src/features/settings/storage.rs` |
| 添加腾讯 Maven 镜像 | 下载与存储 · 预设行 [添加] | 添加 Maven Central 镜像；保留已有规则和优先顺序，完整添加后禁用按钮 |  | `lumilio-ui/src/features/settings/storage.rs` |
| 下载源 | 下载与存储 · 分段：仅官方 / 官方优先 / 镜像优先 | 立即保存；仅官方不访问镜像，其他模式按顺序回退，MCIM 始终位于官方之后 |  | `lumilio-ui/src/features/settings/storage.rs` |
| 镜像规则 | 下载与存储 · 值 + [编辑] 弹窗 | 每行 官方前缀 => 镜像前缀 |  | `lumilio-ui/src/features/settings/storage.rs` |
| 同时下载数 | 下载与存储 · 值 + [编辑] 弹窗 | 自动 / 1–32 |  | `lumilio-ui/src/features/settings/storage.rs` |
| 自动下载方块资源 | 下载与存储 · 开关（默认开） | 立即保存；关闭后不再下载方块资源，已经下载过的照常使用；没下载过时投影预览说明原因 | plan schematic-studio D2/T3/T21 | `lumilio-ui/src/features/settings/storage.rs` |
| 数据目录 | 下载与存储 · 路径 + [在访达中显示] | 不可改（App State） |  | `lumilio-ui/src/features/settings/storage.rs` |
| 存储占用 | 下载与存储 · 游戏 / 共享资源 / Java / 缓存 条形图 | 后台计算后显示 |  | `lumilio-ui/src/features/settings/storage.rs` |
| 清除地图缓存 | 下载与存储 · 按键「清除地图缓存」 | 后台删除种子地图缓存；下次查看时重新计算 |  | `lumilio-ui/src/features/settings/storage.rs` |
| 检查没用的游戏文件 | 下载与存储 · 按键「检查没用的游戏文件」 | 只报告能回收多少，确认后才删；游戏在安装/更新时拒绝 | ADR 0017 | `lumilio-ui/src/features/settings/storage.rs` |
| 清理缓存 | 下载与存储 · 按键「清理缓存」 | 缓存 = 已解压的 natives 与 cache/，下次启动会重建 |  | `lumilio-ui/src/features/settings/storage.rs` |
