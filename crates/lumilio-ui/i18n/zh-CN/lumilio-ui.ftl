## 简体中文：启动器自己的文字。这是回退目录，必须完整；`tr!` 在编译期按它检查。
## 消息 id 写成 页面-部件-含义，例如 settings-general-appearance。

## 通用

common-none = 无
common-not-set = 未设置
common-edit = 编辑
common-edit-more = 编辑…
common-add = 添加
common-auto = 自动
common-more = 更多
common-copy = 复制
common-close = 关闭
common-technical-details = 技术详情
# 打开并选中文件所在位置。按平台取用户熟悉的名字，见 platform::reveal_label。
common-reveal-macos = 在访达中显示
common-reveal-windows = 在文件资源管理器中显示
common-reveal-linux = 在文件管理器中显示
# 列表各项之间的分隔，例如「包装、退出后」。
common-list-separator = 、
# 列表的第一项和总数，例如「-Da 等 3 项」。
common-list-first-of = { $first } 等 { $count } 项

## 语言名称用各自的语言写，在任何界面语言里都一样。

language-simplified-chinese = 简体中文
language-english = English

## 导航

route-home = 首页
route-library = 游戏库
route-discover = 发现
route-activity = 动态
route-accounts = 账户
route-settings = 设置
nav-back = 后退
nav-forward = 前进
nav-add-account = 添加账户
nav-add-account-help = 添加一个离线账户才能进游戏
nav-current-account = 当前账户：{ $name }
nav-switch-account = 切换账户
nav-manage-accounts = 管理账户…
# 账户芯片下拉里一行的说明，例如「Microsoft · 需要重新登录」。
nav-account-needs-sign-in = { $kind } · 需要重新登录
nav-no-game = 还没有游戏
nav-no-game-help = 去游戏库新建一个
nav-current-game = 当前游戏：启动和安装都用它
nav-switch-game = 切换当前游戏
detail-title-modpack = 整合包详情
detail-title-mod = Mod 详情
detail-title-resource-pack = 资源包详情
detail-title-shader = 光影详情

## 游戏库与发现的搜索和筛选

library-search = 搜索游戏
library-all-loaders = 全部加载器
discover-search = 搜索 Modrinth，回车确认
discover-version-search = 搜索版本

## 设置

settings-title = 设置
settings-subtitle = 启动器本身，以及每个游戏默认使用的值
settings-loading = 正在读取设置…
settings-tab-general = 通用
settings-tab-game-defaults = 游戏默认
settings-tab-java = Java
settings-tab-downloads = 下载与存储
settings-tab-about = 关于
settings-tab-plugins = 插件

settings-follow-system = 跟随系统
settings-appearance = 外观
settings-appearance-light = 浅色
settings-appearance-dark = 深色
settings-after-launch = 进入游戏后
settings-after-launch-keep = 保持
settings-after-launch-hide = 隐藏启动器
settings-foreground = 游戏退出后回到前台
settings-foreground-help = 游戏结束时把启动器带回最前面，方便接着选下一个游戏。
settings-motion = 减少动效
settings-motion-reduce = 减少
settings-motion-full = 完整
settings-language = 语言
settings-language-help = 启动器界面的语言。游戏日志和项目介绍保持原文。

settings-memory = 内存
settings-memory-value = 最小 { $min } · 最大 { $max }
settings-memory-help = 本机内存 { $total }，推荐最大内存 { $recommended } MB。留空则交给 Java 决定；每个游戏还可以单独设置。
settings-memory-help-unknown = 留空则交给 Java 决定；每个游戏还可以单独设置。
settings-memory-dialog = 默认内存
settings-memory-intro = 所有游戏默认使用的内存；单个游戏可以覆盖。留空表示不限制。
settings-memory-min = 最小内存（MB）
settings-memory-min-help = 游戏启动时就占用的内存（-Xms）。
settings-memory-max = 最大内存（MB）
settings-memory-max-help = 游戏最多能用的内存（-Xmx）。
settings-memory-placeholder = 不设置
settings-window = 窗口大小与全屏
settings-window-intro = 宽和高需要一起填；都留空则用游戏自己的默认（854 × 480）。
settings-window-width = 宽度
settings-window-height = 高度
settings-window-fullscreen = 全屏启动
settings-window-fullscreen-value = 全屏
settings-fullscreen-off = 关
settings-fullscreen-on = 开
settings-fullscreen-unset = 不设置
settings-one-per-line = 每行一个参数。
settings-jvm = Java 参数
settings-jvm-help = 传给 Java 的附加参数，例如 -XX:+UseG1GC。游戏自己设置了参数时以游戏的为准。
settings-game-args = 游戏参数
settings-game-args-help = 传给游戏本身的附加参数，例如 --demo。
settings-env = 环境变量
settings-env-help = 游戏进程启动时额外带上的环境变量。
settings-env-intro = 每行一个，写成 名称=值。
settings-commands = 启动前、包装与退出后命令
settings-commands-help = 这些命令由你自己填写，会以当前用户的权限运行。
settings-commands-dialog = 命令
settings-commands-intro = 命令以你的用户权限运行，只在你填写后才会执行。可用变量：$INST_ID、$INST_NAME、$INST_DIR（游戏目录）、$INST_JAVA、$INST_MC_VERSION、$INST_LOADER。
settings-command-pre = 启动前命令
settings-command-pre-short = 启动前
settings-command-pre-help = 游戏启动前运行；失败（退出码不为 0）会取消这次启动。
settings-command-pre-placeholder = 例如 ./prepare.sh
settings-command-wrapper = 包装命令
settings-command-wrapper-short = 包装
settings-command-wrapper-help = 放在 Java 命令前面，例如 gamemoderun 或 mangohud。
settings-command-wrapper-placeholder = 例如 gamemoderun
settings-command-post = 退出后命令
settings-command-post-short = 退出后
settings-command-post-help = 游戏退出后运行；失败只会记录，最长运行 60 秒。
settings-command-post-placeholder = 例如 ./cleanup.sh
settings-section-resources = 资源
settings-section-arguments = 参数与环境
settings-section-commands = 命令

settings-java-none = 没有找到 Java
settings-java-none-help = 点“添加 Java”选一个，或在额外搜索目录里加入它所在的文件夹。
settings-java-enable = 启用这个 Java
settings-java-disabled = { $title }（已停用）
settings-java-roots = 额外搜索目录
settings-java-roots-help = 除了常见位置，也会在这些文件夹里找 Java。
settings-java-roots-intro = 每行一个文件夹。每个游戏会按需要自动选择合适的 Java。
settings-java-roots-field = 文件夹
settings-java-roots-count = { $count } 个
settings-java-found = 已发现的 Java
settings-java-rescan = 重新检测
settings-java-install = 下载推荐的 Java
settings-java-add = 添加 Java…

settings-storage-games = 游戏
settings-storage-shared = 共享资源
settings-storage-java = Java
settings-storage-cache = 缓存
settings-mirror-added = 已添加
settings-mirror-not-added = 未添加
settings-bmclapi-help = 游戏资源、Forge、NeoForge、Fabric 和 authlib-injector。添加后可选择“镜像优先”。
settings-mcim-help = Modrinth 和 CurseForge 的信息与文件。始终先试官方，失败再用 MCIM；“仅官方”时不使用。
settings-tencent-maven = 腾讯 Maven
settings-tencent-maven-help = Maven Central 上的 Java 依赖。
settings-download-source = 下载源
settings-download-source-help = 仅官方不访问镜像。其他模式在首选源失败后尝试下一源；MCIM 始终作为官方后的后备。默认官方优先。
settings-source-official-only = 仅官方
settings-source-official-first = 官方优先
settings-source-mirror-first = 镜像优先
settings-mirrors = 镜像规则
settings-mirrors-help = 把官方地址的开头换成镜像地址的开头。
settings-mirrors-intro = 每行一条，写成 官方前缀 => 镜像前缀。
settings-mirrors-field = 规则
settings-mirrors-count = { $count } 条
settings-concurrency = 同时下载数
settings-concurrency-help = 同时下载的文件个数，1 到 32。网络不稳时调小。
settings-concurrency-intro = 留空则由启动器决定。
settings-concurrency-field = 个数（1–32）
settings-data-dir = 数据目录
settings-data-dir-help = 游戏、共享资源和设置都放在这里，不能更改。
settings-reclaim = 检查没用的游戏文件
settings-clear-cache = 清理缓存（{ $size }）
settings-usage-measuring = 正在计算占用…
settings-section-downloads = 下载
settings-section-storage = 存储
settings-section-usage = 占用

settings-version = 版本
settings-updates = 检查更新
settings-updates-help = 启动器自动更新还没有提供。
settings-updates-value = 暂未提供
settings-logs = 启动器日志
settings-logs-help = 下载与安装的记录。
settings-diagnostics = 诊断包
settings-diagnostics-help = 打包版本、设置摘要、Java 列表和各游戏的最近日志；玩家名、UUID 和你的文件夹路径会被替换，启动前/包装/退出后命令只记录有没有填。
settings-diagnostics-export = 导出…
settings-license = 开源许可

settings-plugins-none = 暂无插件
settings-plugins-none-help = 核心插件会在这里显示，你可以按需要开关。
settings-plugin-read-files = 读取游戏的 { $folder } 文件夹
settings-plugin-network = 访问 { $hosts }
settings-plugin-launch-events = 了解游戏的启动和退出状态
settings-plugin-discord = 向本机 Discord 显示游戏状态
settings-plugin-enabled = 已启用
settings-plugin-disabled = 已停用
settings-plugin-failed = 本次运行已停用，重启后重试
settings-plugin-enable = 启用插件
settings-plugin-state = 启用
settings-plugin-permission = 权限
settings-plugin-failure = 失败原因
settings-plugin-failure-value = 本次运行暂停了这个插件
settings-plugin-enable-setting = 启用此设置
settings-plugin-defaults = 默认值
settings-plugin-reset = 恢复默认
