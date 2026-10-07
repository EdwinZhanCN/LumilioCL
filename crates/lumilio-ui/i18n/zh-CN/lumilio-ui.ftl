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
common-cancel = 取消
common-delete = 删除
common-done = 完成
# 一句话里并列的几段之间，例如「cache；natives」。
common-clause-separator = ；

## 时间

time-just-now = 刚刚
time-minutes-ago = { $count } 分钟前
time-hours-ago = { $count } 小时前
time-yesterday = 昨天
time-days-ago = { $count } 天前
time-months-ago = { $count } 个月前
time-years-ago = { $count } 年前

## 加载器

loader-vanilla = 原版
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

## 首页

home-loading = 正在准备
home-continue = 继续
home-continue-eyebrow = 接着上次
home-import = 把原来的游戏带过来
home-create = 新建
home-first-use-eyebrow = 第一次来到这里
home-first-use-title = 先把熟悉的世界放在手边
home-first-use-body = 导入原来的游戏，其余设置之后再慢慢展开。
home-phase-verifying = 检查
home-phase-libraries = 依赖库
home-phase-assets = 资源
home-phase-starting = 启动
home-phase-verifying-headline = 正在检查游戏文件
home-phase-libraries-headline = 正在补齐依赖库
home-phase-assets-headline = 正在准备资源文件
home-phase-starting-headline = 正在启动游戏
home-entering = 正在进入 · { $title }
home-playing-eyebrow = 正在游戏中
# 游戏运行时英雄区下方的一行。
home-playing-caption = { $minutes ->
    [0] 刚刚开始
   *[other] 已经玩了 { $minutes } 分钟
} · 启动器会保持安静
home-stop-game = 结束游戏
home-recovery-eyebrow = 需要看一眼
home-recovery-crashed-title = 游戏意外退出了
home-recovery-failed-title = 上次没有启动成功
home-recovery-interrupted = 上次没有正常结束。
home-recovery-exited-early = 游戏在启动时退出了。
home-recovery-exited-early-code = 游戏在启动时退出了（退出代码 { $code }）。
# phase 是启动步骤的名字，例如「资源」。
home-recovery-stopped-at = 在「{ $phase }」这一步停了下来。
home-recovery-crashed = 游戏意外退出了。
home-recovery-crashed-code = 游戏意外退出了（退出代码 { $code }）。
home-recover = 恢复并继续
home-attention = 需要留意
# 一条提醒，后面还有更多问题时。
home-attention-more = { $detail }（另有 { $more } 个问题）
home-recent = 最近
home-continued = 接着玩
home-record = 游戏记录
home-record-play-time = 游玩时间
home-record-worlds = 世界
home-record-servers = 服务器
home-places-empty = 还没有世界
home-places-empty-help = 按「继续」进入游戏，创建的世界会出现在这里
home-place-enter = 进入
home-place-world = 世界
home-place-last-played = 上次游玩 { $when }
home-place-hardcore = 极限模式
home-place-server = 服务器 · { $address }
home-meta-last-played = { $meta } · 上次游玩于 { $when }
home-meta-never-played = { $meta } · 还没玩过

## 首页英雄区：世界里的说法，可以有游戏里的趣味（设计语言 §8）。

hero-dawn-eyebrow = 主世界
hero-dawn-title = 新的一天，从第一块方块开始
hero-dawn-caption = 太阳是方的，云是平的，一切都刚刚好。
hero-dawn-hud = 时间 { $time }
hero-caves-eyebrow = 洞穴
hero-caves-title = 火把的光，每走一格暗一级
hero-caves-caption = 挖开最后一格石头，岩浆的光会自己涌进来。
hero-caves-hud = 火把 ×{ $torches }
hero-redstone-eyebrow = 红石
hero-redstone-title = 十五格之后，信号就会熄灭
hero-redstone-caption = 同样长的两条线路，只有经过中继器的那一条点亮了灯。
hero-redstone-hud = 信号强度 { $signal }
hero-portal-eyebrow = 下界
hero-portal-title = 四乘五的黑曜石，点燃另一个世界
hero-portal-caption = 打火石一响，紫色的光就洒在了下界岩上。
hero-portal-hud-active = 传送门 已激活
hero-portal-hud-igniting = 传送门 点燃中
hero-portal-hud-inactive = 传送门 未激活
hero-hearth-eyebrow = 营地
hero-hearth-title = 营火还没有熄
hero-hearth-caption = 世界停在你离开的那一刻。
hero-hearth-hud = 营火 光照 { $light }
hero-loading-hud = 区块 { $loaded }/{ $total }

## 游戏库

library-tab-all = 全部游戏
library-tab-favorites = 收藏
library-tab-collections = 合集
library-sort = 排序方式
library-sort-recent = 最近游玩
library-sort-name = 名称
library-sort-created = 创建时间
library-loader = 加载器
library-game-count = { $count } 个游戏
library-loading = 正在读取…
library-empty = 还没有游戏
library-empty-help = 新建一个，或去发现里装一个整合包
library-favorites-empty = 还没有收藏
library-favorites-empty-help = 点卡片上的星标，常玩的游戏会出现在这里
library-no-match = 没有匹配的游戏
library-no-match-help = 换个关键词试试
library-never-played = 还没玩过
library-play = 启动
library-favorite = 收藏游戏
library-unfavorite = 取消收藏
library-menu-play = 开始游戏
library-menu-open = 打开
library-menu-make-current = 设为当前游戏
library-menu-collections = 加入合集…
library-menu-copy = 复制…
library-menu-export = 导出整合包…
library-delete = 删除…
library-new-game = 新建游戏
library-import-pack = 导入整合包
library-import-game = 导入其他启动器的游戏…
library-restore = 从备份恢复…
library-open-folder = 打开游戏库文件夹
library-collections-none = 还没有合集
library-collections-none-help = 把游戏按你的方式归类，比如“生存”“服务器”“整合包”
library-collection-new = 新建合集
library-collection-rename = 改名…
library-collection-delete = 删除合集
library-collection-empty = 这个合集还是空的，在游戏卡片的 ⋯ 菜单里选“加入合集…”
collection-name-required = 请输入名称
collection-name-taken = 已经有一个叫“{ $name }”的合集
collection-name-placeholder = 例如 生存、整合包
collection-new-placeholder = 或者新建一个合集
collection-add-title = 加入合集
collection-add-none = 还没有合集，在下面起个名字就会新建一个
collection-delete-title = 删除合集“{ $name }”？
collection-delete-body = 只会删除这个合集，里面的游戏都还在游戏库里。
game-delete-title = 删除“{ $name }”？
game-delete-body = 游戏目录、存档和历史会一起删除，之后不能找回。
reclaim-title = 清理 { $size } 没用的游戏文件？
reclaim-body = 没有游戏用到它们。删掉后，以后需要时会重新下载。
reclaim-body-kept =
    没有游戏用到它们。删掉后，以后需要时会重新下载。
    没清理的部分：{ $kept }
reclaim-confirm = 清理
collection-rename-title = 合集改名
collection-rename-confirm = 改名
collection-create-confirm = 新建
collection-renamed = 合集已改名为“{ $name }”
collection-created = 已新建合集“{ $name }”
collection-save-failed = 没能保存合集
collection-updated = 合集已更新
collection-update-failed = 没能更新合集
library-import-game-prompt = 选择其他启动器的游戏文件夹
library-import-game-none = 这个文件夹里没找到能导入的游戏
library-import-game-started = 开始导入 { $name }，进度在动态里
library-import-game-done = 已导入「{ $name }」
library-import-game-failed = 没有导入 { $name }
library-restore-prompt = 选择备份文件（.zip）
library-restore-started = 开始恢复备份，进度在动态里
library-restore-done = 已恢复为新游戏「{ $name }」
library-restore-failed = 没能恢复这个备份
game-picker-title = 导入哪个游戏
game-picker-body = 这个文件夹里有不止一个游戏。导入会复制一份玩家文件（Mod、存档、设置），原来的不会被改动。
game-picker-import = 导入

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
