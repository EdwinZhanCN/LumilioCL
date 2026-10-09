## 简体中文：启动器自己的文字。这是回退目录，必须完整；`tr!` 在编译期按它检查。
## 消息 id 写成 页面-部件-含义，例如 settings-general-appearance。

## 通用

common-none = 无
common-not-set = 未设置
common-edit = 编辑
common-edit-more = 编辑…
common-add = 添加
common-auto = 自动
common-show-more = 显示更多
common-show-less = 收起
common-more = 更多
common-copy = 复制
common-close = 关闭
common-technical-details = 技术详情
common-cancel = 取消
common-delete = 删除
common-done = 完成
# 一句话里并列的几段之间，例如「cache；natives」。
common-clause-separator = ；
common-remove = 移除
common-save = 保存
common-retry = 重试
common-open = 打开
common-name = 名称
common-expand = 展开
common-browse = 浏览…
common-restore-defaults = 恢复默认

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
nav-manage-accounts = 账户详情
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
reclaim-kept-installer-libraries = 有 Forge / NeoForge 游戏：它们安装时生成的库文件没有清单，库文件夹先不清理
reclaim-kept-asset-index = 有游戏的资源索引读不到，资源文件先不清理
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
game-picker-origin-minecraft = Minecraft 文件夹
collection-deleted = 已删除合集“{ $name }”
collection-delete-failed = 没能删除合集
library-drop-unsupported = 这里只能放整合包（.mrpack 或 .zip）

## 发现

discover-subtitle = 找到下一次想玩的东西
discover-kind-modpack = 整合包
discover-kind-resource-pack = 资源包
discover-kind-shader = 光影
discover-searching = 正在搜索…
discover-no-source = 没有可用的内容源
discover-no-source-help = 在 设置 › 插件 里打开一个内容源（例如 Modrinth）后再来
discover-offline = 现在是离线的，或连不上 Modrinth
discover-offline-help = 连上网络后再试一次
discover-no-results = 没有结果
discover-no-results-help = 换个关键词或放宽筛选试试
discover-browse-modrinth = 在 Modrinth 中浏览
discover-search-again = 重新搜索
discover-page-size = 显示数量
discover-sort-relevance = 相关度
discover-sort-downloads = 下载量
discover-sort-follows = 关注数
discover-sort-newest = 最新发布
discover-sort-updated = 最近更新
# what 是筛选的名字（游戏版本、加载器），value 是游戏的值，例如「1.21.1」。
discover-locked = { $what }由游戏提供：{ $value }
discover-locked-help = 解锁后能看到不适合这个游戏的内容，安装时仍只装适合它的版本。
discover-unlock = 解锁筛选
discover-sync = 与游戏同步
discover-all-versions = 显示全部版本
discover-open-source = 开源
discover-advanced-remembered = 这里的选择会被记住，下次发现时仍然生效。
discover-hide-installed = 隐藏已安装
discover-filters-not-loaded = 筛选项没有加载
discover-clear-filters = 清除筛选
discover-install = 安装
discover-installing = 安装中…
discover-update = 更新
discover-installed = 已安装
discover-open-modrinth = 在 Modrinth 中打开
discover-copy-link = 复制链接
discover-link-copied = 已复制项目链接
discover-install-started = 开始安装 { $title }，进度在动态里
discover-modpack-installed = 已安装整合包 { $name }
discover-install-failed = 没有装上 { $title }
discover-needs-game = 先在游戏库里新建一个游戏
discover-installed-into = 已把 { $file } 装进 { $game }
discover-dependencies-failed = { $dependencies } 没有装上，{ $title } 可能进不了游戏
environment-client-or-server = 客户端或服务端
environment-client-and-server = 客户端和服务端
environment-client = 客户端
environment-server = 服务端
environment-singleplayer = 单人游戏
environment-dedicated-server = 专用服务器

## 发现的筛选分组与高级排除

discover-exclude-ai-content = AI 生成内容
discover-exclude-ai-content-code = AI 代码
discover-exclude-ai-content-assets = AI 素材
discover-exclude-ai-content-text = AI 文本
discover-exclude-ai-functionality = 生成式 AI 功能
discover-exclude-advertisements = 广告
discover-exclude-system-interactions = 与外部系统交互
discover-exclude-telemetry = 遥测
discover-exclude-telemetry-opt-in = 遥测·选择加入
discover-exclude-telemetry-opt-out = 遥测·可以退出
discover-exclude-telemetry-always-active = 遥测·始终启用
discover-exclude-paid-features = 付费功能
discover-exclude-archived = 已归档
discover-exclude-plugin = 插件
discover-exclude-datapack = 数据包
discover-exclude-epilepsy = 光敏性触发
discover-section-version = 游戏版本
discover-section-loader = 加载器
discover-section-environment = 运行环境
discover-section-license = 许可证
discover-section-advanced = 高级排除
discover-section-categories = 分类
discover-section-features = 特性
discover-section-resolutions = 分辨率
discover-section-performance = 性能影响

## Modrinth 的分类、特性和加载器标签，按 tag-<Modrinth 的名字> 在运行时查找；专有名词不在这里。

tag-adventure = 冒险
tag-cursed = 诅咒
tag-decoration = 装饰
tag-economy = 经济
tag-equipment = 装备
tag-food = 食物
tag-game-mechanics = 游戏机制
tag-library = 库
tag-magic = 魔法
tag-management = 管理
tag-minigame = 小游戏
tag-mobs = 生物
tag-optimization = 优化
tag-social = 社交
tag-storage = 存储
tag-technology = 科技
tag-transportation = 交通
tag-utility = 实用
tag-worldgen = 世界生成
tag-challenging = 挑战
tag-combat = 战斗
tag-kitchen-sink = 大杂烩
tag-lightweight = 轻量
tag-multiplayer = 多人
tag-quests = 任务
tag-audio = 音频
tag-blocks = 方块
tag-core-shaders = 核心着色器
tag-entities = 实体
tag-environment = 环境
tag-fonts = 字体
tag-gui = 界面
tag-items = 物品
tag-locale = 本地化
tag-modded = 模组
tag-models = 模型
tag-realistic = 写实
tag-simplistic = 极简
tag-themed = 主题
tag-tweaks = 调整
tag-vanilla-like = 原版风格
tag-atmosphere = 大气
tag-bloom = 泛光
tag-cartoon = 卡通
tag-colored-lighting = 彩色光照
tag-fantasy = 奇幻
tag-foliage = 植被
tag-path-tracing = 路径追踪
tag-reflections = 反射
tag-semi-realistic = 半写实
tag-low = 低性能影响
tag-medium = 中性能影响
tag-high = 高性能影响
tag-potato = 土豆机
tag-screenshot = 截图用
tag-vanilla = 原版
tag-plugin = 插件
tag-datapack = 数据包
tag-features = 特性

## 账户

# 账户页的页头、空状态与账户行。
account-add-offline = 添加离线账户
account-sign-in-microsoft = 登录 Microsoft
account-menu-third-party = 第三方登录…
account-menu-servers = 认证服务器…
account-empty-title = 还没有账户
account-empty-help = 用 Microsoft 登录可以进入正版服务器；LittleSkin 等第三方认证服务器在右上角 ⋯ 里登录；离线账户不需要登录，名称就是你在游戏里的名字。
account-subtitle-current = 当前：{ $name }（{ $kind }）
account-subtitle-empty = 还没有账户，添加一个才能进游戏
account-needs-sign-in = 需要重新登录
account-chip-current = 当前
account-kind-offline = 离线账户
account-kind-third-party = 第三方账户
account-skin-local = 本地皮肤
account-skin-littleskin = LittleSkin 皮肤
account-skin-site = 皮肤站皮肤
account-detail-custom-uuid = 自定义 UUID
account-copied-uuid = 已复制 UUID
account-menu-copy-uuid = 复制 UUID
account-menu-refresh = 刷新登录
account-menu-remove = 移除…
account-remove-title = 移除账户“{ $name }”？
account-remove-body-signed-in-current = 会忘记这个身份并从系统凭据库删除它的登录信息（第三方账户还会通知服务器作废令牌），不会删除任何游戏或存档。它是当前账户，移除后会改用剩下的第一个。
account-remove-body-signed-in = 会忘记这个身份并从系统凭据库删除它的登录信息，不会删除任何游戏或存档。
account-remove-body-offline-current = 只会忘记这个身份，不会删除任何游戏或存档。它是当前账户，移除后会改用剩下的第一个。
account-remove-body-offline = 只会忘记这个身份，不会删除任何游戏或存档。

# 离线账户的添加对话框。
account-name-label = 名称
account-name-required = 请输入名称
account-name-too-long = 名称最多 { $count } 个字符
account-name-invalid-char = 名称只能用字母、数字和下划线，不能用“{ $character }”
account-name-help = 最多 { $count } 位字母、数字或下划线
account-name-help-first = 最多 { $count } 位字母、数字或下划线。这是第一个账户，会自动设为当前账户
account-name-placeholder = 例如 Steve
account-uuid-placeholder = 留空：由名称决定
account-uuid-help = 游戏用它认出你，存档里的玩家数据也按它保存。留空则由名称决定（同名永远得到同一个）；只能在添加时设置，之后不能更改。
account-uuid-problem = UUID 需要 32 位十六进制数字（可带横线）
account-advanced-expand = 高级选项
account-advanced-collapse = 收起高级选项

# 离线账户的皮肤对话框。
account-skin-title = { $name } 的皮肤
account-skin-kind-default = 默认
account-skin-kind-local = 本地文件
account-skin-kind-littleskin = LittleSkin
account-skin-kind-site = 皮肤站（CustomSkinLoader）
account-skin-model-classic = 经典（宽臂）
account-skin-model-slim = 纤细（窄臂）
account-skin-model-label = 模型
account-skin-skin-label = 皮肤图片
account-skin-skin-placeholder = 皮肤图片（PNG）
account-skin-cape-label = 披风图片
account-skin-cape-placeholder = 披风图片（PNG，可空）
account-skin-api-label = 皮肤站地址
account-skin-api-placeholder = 皮肤站地址（CustomSkinLoader API）
account-skin-browse = 浏览…
account-skin-pick-picture = 选择图片
account-skin-choose-picture = 选一个皮肤图片，或者一个披风图片
account-skin-enter-address = 请输入皮肤站地址
account-skin-save = 保存
account-skin-api-help = 地址下需要有 <玩家名>.json 和 textures/ 目录，LittleSkin 和 Blessing Skin 皮肤站都是这样。
account-skin-default-help = 游戏按玩家 UUID 自己挑一个默认皮肤，启动时不需要额外的东西。
account-littleskin-hint = 你需要在 LittleSkin 上创建一个和这个离线账户同名的角色。之后账户的皮肤就是皮肤站上那个角色所设置的。
account-skin-agent-note = 选了皮肤后，启动游戏时启动器会在本机起一个小小的皮肤服务器，并加载 authlib-injector（第一次会自动下载）。
account-open-littleskin = 打开 LittleSkin

# 登录失败的句子（Microsoft、第三方认证与皮肤站）。
account-auth-declined = 你在浏览器里拒绝了这次登录
account-auth-expired = 代码已经过期，请重新开始登录
account-auth-cancelled = 登录已取消
account-auth-sign-in-required = 登录已失效，需要重新登录
account-auth-no-xbox = 这个 Microsoft 账户还没有 Xbox 档案，请先在 xbox.com 创建一个
account-auth-child = 这是儿童账户，需要家长在 Microsoft 家庭组里允许在线游戏
account-auth-xbox-unavailable = 你所在的地区不提供 Xbox 服务
account-auth-adult-verification = 这个账户需要先在 Xbox 网站完成成年人验证
account-auth-no-game = 这个账户没有 Minecraft Java 版
account-auth-services-refused = Minecraft 服务拒绝了这个启动器的登录，可能这个应用注册还没有通过 Mojang 的审批
account-auth-credential-store = 系统凭据库不可用，登录信息无法安全保存，所以没有登录
account-auth-network = 连不上登录服务，请检查网络后重试
account-auth-protocol = 登录服务的回答不符合预期
account-yggdrasil-network = 无法连接认证服务器。可能是网络问题，请检查设备能否正常上网，或使用代理服务
account-yggdrasil-malformed = 无法解析认证服务器响应，可能是服务器故障
account-yggdrasil-credentials = 用户名或密码错误，或登录次数过多被暂时禁止登录，请稍后再试
account-yggdrasil-session-expired = 登录已经失效，需要重新登录
account-yggdrasil-no-character = 该账户在这个服务器上没有角色
account-yggdrasil-character-deleted = 此角色已被删除
account-yggdrasil-invalid-token = 登录已经失效，请重新登录
account-yggdrasil-migrate = 你的账户需要迁移至微软账户。如果已经迁移，请使用迁移后的微软账户登录
account-skin-error-io = 读不到这个皮肤文件
account-skin-error-picture = 无法识别的皮肤文件，需要是 PNG 图片
account-skin-error-network = 连不上皮肤站，请检查网络和地址
account-skin-error-malformed = 皮肤站的回答不符合预期
account-skin-error-invalid-api = 皮肤站地址不是一个有效的地址
account-failure-sign-in-required = 账户 { $name } 需要重新登录
account-failure-injector = 无法下载 authlib-injector。可能是网络问题，请检查网络、尝试切换下载源或使用代理服务
account-failure-no-pending = 这次登录已经失效，请重新登录
account-failure-duplicate = 已经有叫“{ $name }”的账户了（名称不分大小写）
account-failure-duplicate-uuid = 这个 UUID 已经被另一个账户使用
account-failure-profile = 名称或 UUID 不符合要求
account-failure-save = 没能保存账户

# 认证服务器对话框。
account-server-label = 认证服务器
account-server-add-title = 添加认证服务器
account-server-add-menu = 添加认证服务器…
account-server-address-placeholder = 例如 littleskin.cn 或认证服务器的 API 地址
account-server-find = 查找
account-server-builtin = { $url } · 内置
account-server-accounts = { $url } · { $count } 个账户
account-server-already-listed = 这个服务器已经在列表里了
account-server-remove-title = 移除认证服务器“{ $name }”？
account-server-remove-body = 只会忘记这个服务器，之后可以再添加。
account-server-remove-body-accounts = 会忘记这个服务器，并移除在它上面登录的 { $count } 个账户（连同它们在系统凭据库里的登录信息）。游戏和存档不受影响。

# 第三方账户登录对话框。
account-third-party-title = 登录第三方账户
account-login-username = 用户名
account-login-email = 邮箱
account-password-label = 密码
account-password-hint = 密码只会发给你选的认证服务器；启动器不保存它，只把服务器发来的令牌放进系统凭据库。
account-http-warning = 警告：此服务器使用不安全的 HTTP 协议，你的密码在登录时会被明文传输。
account-sign-in-action = 登录
account-choose-character-action = 选择
account-choose-character = 这个账户有多个角色，选一个用来玩

# Microsoft 登录对话框。
account-microsoft-title = 登录 Microsoft
account-microsoft-sign-in = 在浏览器中登录
account-microsoft-retry = 再试一次
account-microsoft-cancel-sign-in = 取消登录
account-microsoft-intro = 点“在浏览器中登录”，在打开的页面输入一个代码，再回到这里。
account-microsoft-storage-note = 登录信息只保存在系统凭据库里，不会写进启动器的文件。
account-microsoft-requesting = 正在向 Microsoft 要一个登录代码…
account-microsoft-code-lead = 在 { $address } 输入这个代码，然后在页面上登录并确认：
account-microsoft-copy-code = 复制代码
account-microsoft-reopen = 重新打开页面
account-microsoft-waiting = 正在等你在浏览器里完成登录…

# 账户操作的提示消息。
account-signed-in = 已登录 { $name }
account-added = 已添加账户 { $name }
account-server-added = 已添加认证服务器 { $name }
account-server-removed = 已移除认证服务器
account-skin-saved = 皮肤已保存，下次启动时生效
account-refreshed = 登录已刷新
account-switch-failed = 没能切换账户
account-removed = 已移除账户 { $name }
account-remove-failed = 没能移除账户

## 动态

# 任务进行速度，例如「3.2 MB/秒」。
activity-rate-bytes = { $rate }/秒
activity-rate-files = { $rate } 个文件/秒
activity-eta-under-minute = 不到 1 分钟
activity-eta-minutes = 约 { $count } 分钟
activity-eta-hours-minutes = 约 { $hours } 小时 { $minutes } 分钟
# 一条任务行右侧的剩余时间，left 是「约 3 分钟」这样的短语。
activity-remaining = 剩余{ $left }
activity-status-running = 进行中
activity-status-done = 已完成
activity-status-failed = 失败
activity-status-cancelled = 已取消
activity-empty-title = 还没有动态
activity-empty-help = 下载和安装会出现在这里
activity-tab-empty = 这一类里没有动态
activity-clear-finished = 清除已完成
activity-none-running = 现在没有进行中的事情
activity-running-count = { $count } 件事正在进行
# 启动时恢复档的说明，最要紧的一条在前。
activity-recovery-library = 游戏库文件无法读取，已保留原文件并重新开始
activity-recovery-library-candidates = 游戏库文件无法读取，已保留原文件并重新开始；磁盘上找到 { $count } 个可能的游戏目录
activity-recovery-settings = 设置文件无法读取，已保留原文件并使用默认设置
activity-recovery-stuck = 上次中断的操作还没能收尾，文件已保留，下次启动会再试
activity-recovery-conflict = 发现无法自动处理的中断记录，相关文件没有被改动
activity-recovery-restore-rolled-back = 上次快照恢复被中断，已回到恢复前的样子
activity-recovery-session-interrupted = 启动器上次在游戏运行时退出，那次游玩的结果未知
activity-recovery-profile-missing = 有游戏的目录不见了，可以在诊断里查看
activity-recovery-delete-rolled-back = 上次删除被中断，游戏已原样保留
activity-recovery-delete-completed = 上次中断的删除已经完成
activity-recovery-publish-completed = 上次中断的导入或复制已经完成
activity-recovery-publish-discarded = 上次中断的导入或复制没有完成，已清理，可以重新开始
activity-recovery-log-skipped = 动态记录里有 { $count } 行无法读取，已跳过
activity-recovery-more = { $headline }（另有 { $more } 项恢复记录）
# 失败或取消的任务重新发起。
activity-retry-started = 已重新开始，进度在动态里
activity-retry-ok = 这次成功了
activity-retry-failed = 还是没有成功
# Activity 的分类页签。
activity-tab-all = 全部
activity-tab-download = 下载
activity-tab-install = 安装
activity-tab-update = 更新
activity-tab-repair = 修复

## 动态任务：core 只给出动作和对象，这里把它说成一句话
task-install-game = 安装 { $subject }
task-repair-game = 修复 { $subject }
task-change-version = 更换 { $subject } 的版本
task-export-world = 导出 { $subject }
task-download-content = 下载 { $subject }
task-install-content = 安装 { $subject }
task-switch-content-version = 切换 { $subject } 的版本
task-import-game = 导入 { $subject }
task-backup-game = 备份 { $subject }
task-restore-backup = 恢复备份 { $subject }
task-install-java = 安装 Java { $subject }
task-install-java-any = 安装 Java
task-copy-game = 复制 { $subject }
task-install-modpack = 安装整合包 { $subject }
task-import-modpack = 导入整合包 { $subject }

## 新建游戏

new-game-loader-stable = 稳定版
new-game-loader-latest = 最新版
new-game-loader-other = 其他
new-game-install-help = 先下载好，第一次启动更快；不下载也能玩，开始游戏时会自动补齐。
new-game-channel-snapshot = 快照
new-game-channel-pre-release = 预发布
new-game-channel-candidate = 候选
new-game-channel-old = 远古
new-game-loader-tag-stable = 稳定
new-game-loader-tag-testing = 测试
new-game-default-name = 新的{ $loader }游戏
new-game-change-warning = 已装好的 Mod 可能和新的游戏版本或加载器不兼容。世界和设置不会被改动；建议先建一个快照，出问题时可以恢复。
new-game-pick-game-version = 选择游戏版本
new-game-pick-loader-version = 选择加载器版本
new-game-resolved-note = 将安装 { $loader } { $version }
new-game-change = 更换
new-game-create = 创建
new-game-change-title = 更换游戏版本与加载器
new-game-loader-unsupported = { $loader } 还不支持这个游戏版本，换一个版本试试
new-game-loader-version-label = 加载器版本
new-game-install-now = 创建后立即下载游戏文件
new-game-snapshot-first = 先建快照
new-game-read-game-versions = 读不到版本列表
new-game-read-loader-versions = 读不到 { $loader } 的版本
new-game-loader-unsupported-install = { $loader } 还不能安装，支持正在路上
new-game-create-failed = 没有创建成功，可以再试一次
new-game-created-downloading = 已创建 { $name }，正在下载游戏文件
new-game-created = 已创建 { $name }
new-game-download-failed = 游戏文件没有下载完，开始游戏时会再试
new-game-import-pack-prompt = 选择整合包（.mrpack，或 MultiMC / Prism 的 .zip）
new-game-import-pack-done = 已导入整合包 { $name }

## 项目详情

project-tab-about = 介绍
project-tab-versions = 版本
project-tab-gallery = 画廊
project-channel-release = 正式版
project-channel-beta = 测试版
project-channel-alpha = 早期版
project-offline = 连不上 Modrinth
project-offline-help = 检查网络后再打开一次
project-gallery-empty = 这个项目没有画廊
project-open-in-browser = 在浏览器中打开
project-switch-version = 切换版本
project-install-as-new-game = 安装为新游戏
project-install-into = 安装到 { $name }
project-switch = 切换
project-add-game-version = 添加游戏版本
project-versions-empty = 还没有可用的版本
project-version-unfit = 不适用于当前游戏
project-column-channel = 渠道
project-column-published = 发布
project-save-as = 另存为
project-download-version-file = 下载这个版本的文件 · { $size }
project-versions-no-match = 没有符合筛选的版本
project-versions-no-match-help = 放宽筛选试试
project-save-started = 开始下载 { $title }，进度在动态里
project-saved-to = 已保存到 { $path }
project-save-failed = 没有下载 { $title }
project-update-started = 开始更新 { $title }，进度在动态里
project-updated-to = 已更新为 { $file }（{ $instance }）
project-update-failed = 没有更新 { $title }
project-group-ancient = 所有远古版本
project-group-alpha-beta = 所有 Alpha 和 Beta 版本
project-group-alpha = 所有 Alpha 版本
project-group-beta = 所有 Beta 版本
project-group-pre-alpha = 所有 Pre-alpha 版本

## 依赖提示

dependency-install-alone = 只安装它
dependency-install-together = 一起安装（{ $count } 项）
dependency-needs-more = { $mod_title } 还需要这些
dependency-about = 关于 { $mod_title }
dependency-no-version = { $title }（没有适合这个游戏的版本）
dependency-needed = 需要：没有它们，这个 Mod 多半进不了游戏。取消勾选的不会安装。
dependency-optional = 可选：装上能多一些功能，不装也能用。
dependency-conflicts = 它声明和已装的 { $conflicts } 不兼容，一起用可能进不了游戏。

## 导出整合包

export-name-required = 请输入整合包名称
export-version-required = 请输入版本号
export-include-required = 至少选一项要放进整合包的内容
export-version-placeholder = 例如 1.0.0
export-summary-placeholder = 一句话介绍（可空）
export-ok = 选择位置并导出
export-title = 导出整合包
export-version-label = 版本号
export-summary-label = 简介
export-format-label = 格式
export-include-label = 放进整合包的内容
export-note = Modrinth 上有的文件按地址列出，其余的会直接放进整合包。存档和日志默认不放。

## 版本选择

version-picker-no-match = 没有匹配的版本
version-picker-loading = 正在读取版本…

## 模型预览

model-title = 3D 投影预览
model-open = 打开 3D 预览
model-error-no-gpu = 这台电脑没有可用的图形设备，无法显示 3D 预览
model-error-parse = 读不了这个投影文件
model-error-pack = 读不了游戏的贴图包
model-error-mesh = 没能生成 3D 预览
model-error-render = 没能绘制 3D 预览
model-undrawable = 有 { $count } 种方块这个游戏版本画不出来，预览里没有它们
model-read-failed = 没能读取 3D 预览
model-needs-game = 安装这个游戏后，就能用它的贴图预览投影
model-capture-unsupported = 当前窗口系统暂不支持鼠标捕获，请使用 Orbital 模式
model-capture-failed = 没能捕获鼠标，点击画面重试
model-capture-ended = 鼠标捕获已结束，点击画面重新进入
model-loading = 正在生成 3D 预览…
model-aria = 3D 投影预览；Orbital 方向键旋转、加减键缩放；Explore 点击或 Enter 捕获鼠标，WASD 移动、空格上升、Shift 下降，Esc 释放；R 复位
model-reset = 复位视角
model-hint-orbital = 拖拽旋转 · 滚轮缩放 · 方向键旋转 · + / − 缩放
model-hint-explore-captured = WASD 移动 · 空格上升 · Shift 下降 · Esc 释放鼠标
model-hint-explore = 点击画面或按 Enter 进入 · WASD 移动 · 空格上升 · Shift 下降

## 占位页

placeholder-library-body = 游戏会在这里安静地聚在一起。
placeholder-discover-title = 还没有发现
placeholder-discover-body = Mod、资源包和光影会在这里出现。
placeholder-activity-title = 没有进行中的事情
placeholder-activity-body = 下载、安装和修复完成后，会在这里留下痕迹。
placeholder-accounts-body = 添加一个离线账户，就能进入游戏。
placeholder-settings-title = 设置正在加载
placeholder-settings-body = 启动器与游戏的默认值会在这里。

## 光敏性提示

photosensitivity-title = 关于光敏性筛选
photosensitivity-body = 这个筛选只会排除作者自己声明了有闪烁、频闪等光敏性内容的项目。没有声明的项目不会被排除，所以它不能保证内容对光敏性癫痫患者是安全的。
photosensitivity-dismiss = 知道了，不再提示
photosensitivity-ok = 知道了

## 插件设置

plugin-setting-title = 编辑{ $label }
plugin-setting-integer = 请输入整数
plugin-setting-integer-range = 请输入 { $min } 到 { $max } 之间的整数
plugin-setting-choice-only = 这个设置请在设置行里选择
plugin-setting-save-failed = 没能保存这个设置，请重试。

## 启动

launch-already-running = 已经有游戏在运行

## 日志

logs-saved = 已保存到 { $path }（名字和路径已隐去）
logs-export-failed = 没能导出日志

## 应用

app-quit = 退出 LumilioCL

## 游戏详情

instance-title = 游戏详情
instance-tab-overview = 概览
instance-tab-content = 内容
instance-tab-worlds = 世界
instance-tab-screenshots = 截图
instance-tab-history = 历史
instance-tab-diagnostics = 诊断
# 「设置」标签与路由同名，复用 route-settings。

instance-play = 启动游戏
instance-install-files = 安装游戏文件
instance-repair-files = 修复游戏文件
instance-create-snapshot = 创建快照…
instance-copy-game = 复制这个游戏…
instance-backup-game = 完整备份…
instance-delete-game = 删除游戏…

instance-read-failed = 没有读到这个游戏，可以重试
instance-read-versions-help = 可以关掉再试一次
instance-save-failed-retry = 没有保存成功，可以重试
instance-save-failed-draft = 没有保存成功，输入已保留，可以重试
instance-name-saved = 名称已保存
instance-memory-saved = 内存已保存，下次启动时生效
instance-write-failed = 没有成功，游戏保持原样，可以稍后重试

instance-import-content-prompt = 选择要添加的文件
instance-import-world-prompt = 选择世界的 .zip
instance-export-unavailable = 没能读取游戏的文件，无法导出
instance-saved-to = 已保存到 { $path }
instance-exported-to = 已导出到 { $path }
instance-backed-up-to = 已备份到 { $path }
instance-save-failed = 没有保存成功

instance-runtime-changed = 已更换为 { $runtime }
instance-game-copied = 已复制为「{ $name }」
instance-game-deleted = 游戏已删除
instance-files-installed = 游戏文件已安装
instance-files-repaired = 游戏文件已检查，缺的和损坏的已补上
instance-java-installed = Java 已安装

instance-content-add-files = 添加文件
instance-content-browse-mod = 浏览 Mod
instance-content-browse-resource-pack = 浏览资源包
instance-content-browse-shader = 浏览光影
instance-content-empty = 还没有{ $noun }
instance-content-empty-help = 点「浏览」找一个装进来，或者添加本地文件
instance-content-refresh-tip = 重新读取并识别
instance-content-update-all-count = 全部更新（{ $count }）
instance-content-no-match = 没有符合的条目
instance-content-no-match-help = 换个关键词或筛选试试
instance-content-source-note = 来源信息暂时不可用{ $reason }；文件照常可以启停和删除。
instance-content-filter-all = 全部
instance-content-filter-updates = 有更新
instance-content-filter-disabled = 已停用
instance-content-filter-unknown = 未识别
instance-content-enable = 启用
instance-content-disable = 停用
instance-content-clear-selection = 清除选择
instance-content-selected = 已选 { $count } 项
instance-content-delete-title-many = 删除这 { $count } 个文件？
instance-content-delete-body = 文件会从这个游戏里移走，删除记录会写进历史。
instance-content-update-title = 更新 { $count } 个文件？
instance-content-update-body = 每个都会换成兼容这个游戏的最新版本；换版本可能让游戏出问题，必要时先建快照。
instance-content-update-all = 全部更新
instance-content-unchanged = 没有需要改动的文件
instance-content-changed-enabled = 已启用 { $count } 个文件
instance-content-changed-disabled = 已停用 { $count } 个文件
instance-content-changed-deleted = 已删除 { $count } 个文件
instance-content-switched = 已换成 { $name }
instance-content-updated = 已更新 { $count } 个文件
instance-content-update-partial =
    { $ok } 个更新成功，{ $failed } 个没有成功
    { $detail }
instance-content-added = 已添加 { $count } 个文件
instance-content-add-partial =
    添加了 { $ok } 个，{ $failed } 个没有添加
    { $detail }
instance-content-local-file = 本地文件
instance-content-unknown-version = 未知
instance-content-link-copied = 链接已复制
instance-content-channel-release = 正式
instance-content-channel-beta = 测试
instance-content-channel-alpha = 内测
instance-content-search-versions = 查找版本
instance-content-switch-to = 切换到 { $version }
instance-content-switch-title = 切换版本 · { $title }
instance-content-switch-note = 换版本可能让游戏出问题，必要时先在「历史」里建一个快照。
instance-content-current = 当前
instance-content-latest = 最新
instance-content-incompatible = 不兼容
instance-content-show-incompatible = 显示不兼容的版本
instance-content-version-meta = { $loaders } · { $versions }
instance-content-version-meta-incompatible = { $loaders } · { $versions } · 和这个游戏不兼容
instance-content-no-changelog = 这个版本没有写更新日志

instance-world-imported = 已导入「{ $name }」
instance-world-copied = 已复制为「{ $name }」
instance-world-backed-up = 已备份「{ $name }」，可以在历史的快照里恢复
instance-world-deleted = 世界已删除
instance-server-saved = 已保存「{ $name }」
instance-server-added = 已添加「{ $name }」
instance-server-deleted = 服务器已删除
instance-servers-reordered = 已调整顺序
instance-screenshot-copied = 已复制图片
instance-screenshot-copy-failed = 没有复制成功
instance-screenshot-deleted = 截图已删除
instance-snapshot-created = 快照已创建
instance-snapshot-restored = 已恢复快照
instance-snapshot-deleted = 快照已删除
instance-snapshot-manual = 手动快照
instance-snapshot-manual-backup = 手动备份

instance-plugin-unavailable = 这个插件现在不可用
instance-plugin-action-failed = 没有完成

instance-rename-title = 重命名
instance-rename-help = 改名保留游戏目录、收藏和历史记录。
instance-rename-required = 请输入游戏名称
instance-memory-title = 编辑内存
instance-min-memory-help = 游戏启动时就占用的内存（-Xms）。留空则跟随启动器默认；默认也没有时由 Java 决定。
instance-max-memory-help = 游戏最多能用的内存（-Xmx）。留空则跟随启动器默认；默认也没有时由 Java 决定。
instance-memory-help-footer = 留空则跟随默认，下次启动时生效。
instance-memory-reset = 全部跟随默认
instance-server-edit-title = 编辑服务器
instance-server-add-title = 添加服务器
instance-server-address = 地址
instance-server-address-help = 主机名或 IP，端口可选（默认 25565），例如 mc.example.com 或 mc.example.com:25570。
instance-snapshot-title = 创建快照
instance-snapshot-note = 备注
instance-snapshot-scope = 范围
instance-snapshot-scope-all = 全部世界与设置
instance-snapshot-scope-world = 只备份世界“{ $name }”
instance-copy-title = 复制游戏
instance-copy-confirm = 开始复制
instance-copy-name = 新游戏名称
instance-copy-name-required = 请输入新游戏的名称
instance-copy-name-suffix = { $name } 副本
instance-copy-include-worlds = 同时复制存档
instance-copy-help = 日志与崩溃报告不会复制；收藏、游玩时间、历史和快照从零开始。
instance-delete-body = 游戏目录、存档和历史会一起删除；中途中断也会在下次启动时补完或恢复。
instance-memory-invalid = 请输入整数 MB，留空可继承默认值
instance-memory-range = 内存需在 1–1048576 MB 内，最小值不能超过生效的最大值
instance-memory-inherit = 跟随默认 · { $value } MB
instance-memory-java = 由 Java 决定

instance-field-game-name = 游戏名称
instance-field-inherit-default = 继承默认
instance-field-new-game-name = 新游戏的名称
instance-field-search = 搜索
instance-field-snapshot-note = 备注（可空）
instance-field-search-worlds = 搜索世界
instance-field-server-name = 服务器名称
instance-field-server-address = 地址，例如 mc.example.com:25565
instance-field-search-logs = 搜索日志
instance-field-search-files = 搜索文件

instance-diagnostic-problems = 问题
instance-diagnostic-logs = 日志
instance-diagnostic-files = 文件
instance-problems-none = 没有发现问题
instance-problems-none-help = 游戏现在看起来一切正常
instance-diagnostics-game-dir = 游戏目录
instance-diagnostics-path = 游戏目录 / { $folder }
instance-files-up = 上一级
instance-files-empty = 这个文件夹是空的
instance-files-no-match = 没有匹配的文件
instance-files-folder-meta = 文件夹 · { $time }
instance-files-reveal = 显示
instance-log-live = 实时输出
instance-log-latest = 最近日志 · latest.log
instance-log-crash = 崩溃报告 · { $name }
instance-log-source-label = 日志来源
instance-log-source-search = 搜索日志来源
instance-log-level-label = 日志级别
instance-log-level-prefix = 级别 ·{ " " }
instance-log-level-placeholder = 未选择级别
instance-log-level-error = 错误
instance-log-level-warn = 警告
instance-log-level-info = 信息
instance-log-level-debug = 调试
instance-log-file-meta = { $name } · { $time }
instance-log-crash-meta = 崩溃报告 · { $name } · { $time }
instance-log-analysis-action = 崩溃分析…
instance-log-copied = 已复制日志
instance-log-copy-empty = 没有可复制的日志行
instance-log-export-action = 导出…
instance-log-read-failed = 没有读到这份日志
instance-log-reading = 正在读取日志…
instance-log-none = 还没有日志
instance-log-none-help = 选择其他来源，或运行游戏后再查看
instance-log-no-match = 没有符合条件的日志行
instance-log-line-count = { $count } 行
instance-log-bottom = 回到底部
instance-analysis-running = 正在分析…
instance-analysis-failed = 没能完成分析
instance-analysis-source = 来源：{ $source }
instance-analysis-none = 未识别到已知崩溃原因
instance-analysis-none-help = 这不表示日志没有问题。
instance-analysis-snapshot = 日志快照（已脱敏）：
instance-analysis-copy = 复制技术详情
instance-analysis-title = 崩溃分析

# 游戏详情：概览、内容、世界、截图、历史与设置。
instance-field-game-version = 游戏版本
instance-overview-install-record = 安装记录
instance-overview-installed-help = 曾完成安装，启动时仍会检查文件。
instance-overview-pending-help = 尚未完成安装，首次启动时准备游戏文件。
instance-overview-pending = 待安装
instance-overview-size = 占用空间
instance-overview-size-help = 这个游戏自己的文件；多个游戏共用的游戏文件不算在内。
instance-overview-size-computing = 计算中…
instance-overview-size-failed = 读不到
instance-section-read-failed = 没有读到这部分内容
instance-section-retry-help = 可以重试
instance-action-install = 安装
instance-action-install-java = 安装 Java
instance-action-add-account = 去添加
instance-action-repair = 修复
instance-action-change = 更换…
instance-action-go-content = 去内容
instance-action-go-adjust = 去调整
instance-view-logs = 查看日志
instance-problem-not-installed = 游戏文件还没有安装
instance-problem-not-installed-help = 可以现在安装，也可以首次启动时自动准备
instance-problem-loader-unsupported = 暂时不能启动 { $loader }
instance-problem-loader-unsupported-help = 这个加载器还没有支持，可以改用 Fabric、Quilt 或原版
instance-problem-no-java = 没有找到可用的 Java
instance-problem-java-required = 这个版本需要 Java { $major }
instance-problem-install-java-help = 请安装 Java 后重试
instance-problem-no-account = 还没有选择账户
instance-problem-no-account-help = 离线游玩需要一个玩家名
instance-problem-damaged = 有游戏文件缺失或损坏
instance-problem-damaged-help = 共 { $count } 个，重新安装会补回它们
instance-problem-wrong-loader = 有 Mod 不是为当前加载器制作的
instance-problem-duplicate-mods = 同一个 Mod 装了不止一份
instance-problem-low-memory = 最大内存偏低
instance-problem-low-memory-help = 当前上限 { $max } MB，游戏可能因内存不足而崩溃
instance-problem-last-session = 上一次游玩没有顺利结束
instance-history-view-all = 查看全部
instance-overview-recent-changes = 最近变更
instance-problems-status = 状态
instance-change-content-added = 添加了
instance-change-content-removed = 移除了
instance-change-content-enabled = 启用了
instance-change-content-disabled = 停用了
instance-change-content-updated = 更新了
instance-change-settings = 修改了设置
instance-change-game-version = 更换了游戏版本
instance-change-repaired = 修复了
instance-change-world-copied = 复制了世界
instance-change-world-deleted = 删除了世界
instance-change-world-imported = 导入了世界
instance-change-snapshot-created = 创建了快照
instance-change-snapshot-restored = 恢复了快照
instance-change-snapshot-deleted = 删除了快照
instance-outcome-clean = 正常结束
instance-outcome-crashed = 异常退出
instance-outcome-failed-to-start = 没能启动
instance-outcome-stopped = 被手动停止
instance-outcome-failed-to-prepare = 准备阶段失败
instance-outcome-cancelled = 启动前取消
instance-outcome-interrupted = 启动器中途退出，结果未知
instance-export-offline = 没能连上 Modrinth，{ $bundled } 个文件都直接放进了整合包。已导出到 { $path }
instance-export-linked = 已导出到 { $path }：{ $linked } 个文件按地址列出，{ $bundled } 个放在包里
instance-duration-minutes = { $count } 分钟
instance-duration-hours-minutes = { $hours } 小时 { $minutes } 分钟
instance-confirm-delete-world = 删除世界“{ $name }”？
instance-world-delete-body = 世界和里面的东西都会删除，之后不能找回。想留个后手的话，先创建备份。
instance-confirm-delete-screenshot = 删除截图“{ $file }”？
instance-screenshot-delete-body = 文件会从游戏的 screenshots 文件夹里删除，之后不能找回。
instance-confirm-delete-server = 删除服务器“{ $name }”？
instance-server-delete-body = 只从这个游戏的列表里移除，不影响服务器本身；之后可以重新添加。
instance-snapshot-untitled = 这份快照
instance-confirm-restore-snapshot = 恢复“{ $label }”？
instance-snapshot-restore-body = 恢复会用快照替换现有的世界和设置，当前状态会先自动存一份；中途失败或退出会回到恢复前的样子。
instance-snapshot-restore = 恢复
instance-confirm-delete-snapshot = 删除快照“{ $label }”？
instance-snapshot-delete-body = 删除后不能再恢复到这个时间点。
instance-history-skipped = 有 { $count } 行记录无法读取，已跳过
instance-history-sessions-empty = 还没有游玩记录
instance-history-changes-empty = 还没有变更记录
instance-snapshot-create-now = 现在创建快照
instance-snapshots-empty = 还没有快照
instance-snapshots-empty-help = 快照保存所有世界和游戏设置，恢复失败会自动回到恢复前
instance-history-snapshot-scope-world = 世界 { $name }
instance-content-kind-mod = Mod
instance-content-kind-resource-pack = 资源包
instance-content-kind-shader = 光影
instance-history-changes = 变更
instance-history-sessions = 游玩记录
instance-history-snapshots = 快照
instance-worlds-world = 世界
instance-worlds-server = 服务器
instance-refresh = 刷新
instance-open-folder = 打开文件夹
instance-screenshots-empty = 还没有截图
instance-screenshots-empty-help = 在游戏里按 F2 截图，拍下的画面会出现在这里
instance-screenshot-no-preview = 无法预览
instance-screenshot-copy = 复制图片
instance-server-checking = { $address } · 正在检查…
instance-server-offline = { $address } · 无法连接
instance-server-online-of = { $online }/{ $max } 在线
instance-server-online = { $online } 在线
instance-servers-empty = 还没有服务器
instance-servers-empty-help = 添加后，游戏的多人游戏列表里就有它
instance-server-move-up = 上移
instance-server-move-down = 下移
instance-server-copy-address = 复制地址
instance-server-copied = 已复制地址
instance-server-enter = 进入
instance-server-old-version = 这个游戏版本不能直接进入服务器，请从多人游戏列表进入
instance-servers-running = 游戏正在运行，服务器列表先不能修改；结束游戏后再来。
instance-servers-help = 这里的改动就是游戏里的多人游戏列表。游戏运行时不能修改。
instance-server-name-address-required = 请输入名称和地址
instance-world-drop-unsupported = 这里只能放世界的 .zip
instance-world-drop-one = 一次只导入一个世界，先导入了第一个
instance-server-refresh = 刷新状态
instance-server-add = 添加服务器
instance-world-import = 导入世界
instance-worlds-empty = 还没有世界
instance-worlds-empty-help = 进入游戏创建的世界会出现在这里，也可以导入一个 .zip
instance-world-playing = 正在游玩
instance-world-damaged = 存档信息无法读取
instance-world-played-at = { $when }游玩
instance-world-enter = 进入
instance-world-old-version = 这个游戏版本不能直接进入世界，请从主菜单进入
instance-world-copy = 复制
instance-world-backup = 创建备份
instance-world-export = 导出为 .zip…
instance-worlds-running = 游戏正在运行，世界先不能复制、备份、导出或删除；结束游戏后再来。
instance-worlds-help = 复制和导入总是另存为新世界，不会覆盖已有的世界。游戏运行时不能修改。
instance-worlds-no-match = 没有匹配的世界
instance-load-failed = 没有读到游戏
instance-loading = 正在读取游戏…
instance-plugin-unavailable-title = 这个标签现在不可用
instance-plugin-unavailable-help = 插件已关闭，或者出了问题
instance-back-to-top = 返回顶部
instance-plugin-confirm-title = { $label }？
instance-plugin-confirm-body = 这个操作做了就撤不回来。
instance-settings-window-fullscreen = { $size } · 全屏
instance-settings-window-custom = { $value }（已自定义）
instance-settings-window-help = 宽和高需要一起填；留空跟随启动器的默认。
instance-settings-window-intro = 只改这个游戏；留空或选“跟随默认”就用启动器的默认值。
instance-settings-follow-default = 跟随默认
instance-settings-after-help = 这个游戏开始运行后，启动器窗口怎么处理。
instance-settings-launcher-window = 启动器窗口
instance-settings-quick-play = 直接进入
instance-settings-quick-help = 开始游戏后直接进入一个世界或服务器。较旧的游戏版本不能直接进入单人世界。
instance-settings-quick-intro = 世界名是存档文件夹的名字；服务器写成 地址 或 地址:端口。
instance-settings-type = 类型
instance-settings-target = 目标
instance-settings-target-placeholder = 世界名或服务器地址
instance-settings-version-help = 更换前建议先建一个快照；世界和设置不会被改动。
instance-settings-loader-help = 已装好的 Mod 可能和新的加载器不兼容。
instance-settings-game-files = 游戏文件
instance-settings-files-help = 逐个核对游戏文件，缺的或损坏的会重新下载；世界、Mod 和设置不会被改动。
instance-settings-not-downloaded = 未下载
instance-settings-java-help = 自动选择会按这个游戏需要的 Java 版本挑一个已发现的；也可以指定一个。
instance-settings-java-auto = 自动选择
instance-settings-java-intro = 留空表示自动选择。指定时填 java 程序或 JDK 文件夹的路径；已发现的 Java 在“设置 › Java”里。
instance-settings-path = 路径
instance-settings-java-pick = 选择 java 程序或 JDK 文件夹
instance-settings-jvm-help = 设置后替换启动器默认的 Java 参数；留空则跟随默认。
instance-settings-jvm-intro = 每行一个参数。留空则跟随启动器默认。
instance-settings-args = 参数
instance-settings-machine-memory = 本机内存
instance-settings-machine-memory-help = 推荐的最大内存是本机内存的一半，不超过 8 GB。
instance-settings-machine-memory-value = { $total } · 推荐最大 { $recommended } MB
instance-settings-game-args-help = 传给游戏本身的参数。自己设置后替换默认；设置为空表示这个游戏不用任何参数。
instance-settings-source = 来源
instance-settings-env-help = 游戏进程额外带上的环境变量。自己设置后替换默认。
instance-settings-variables = 变量
instance-settings-commands-value = 启动前：{ $pre } · 包装：{ $wrapper } · 退出后：{ $post }
instance-settings-commands-help = 命令由你自己填写，会以当前用户的权限运行。
instance-settings-commands-intro = 留空跟随启动器默认；填 - 表示这个游戏不使用。命令以你的用户权限运行。可用变量：$INST_ID、$INST_NAME、$INST_DIR、$INST_JAVA、$INST_MC_VERSION、$INST_LOADER。
instance-settings-command-pre-help = 失败（退出码不为 0）会取消这次启动。
instance-settings-command-wrapper-help = 放在 Java 命令前面。
instance-settings-command-post-help = 失败只会记录，最长运行 60 秒。
instance-settings-own = 自己设置
instance-settings-followed = 跟随默认 · { $value }
instance-settings-quick-none = 无（进入主菜单）
instance-settings-quick-world = 世界 · { $name }
instance-settings-quick-server = 服务器 · { $address }
instance-settings-command-none = 不使用
instance-settings-quick-target-required = 请填写世界名或服务器地址
instance-settings-quick-world-empty = 世界名不能为空
instance-settings-quick-world-invalid = 世界名不是一个有效的存档文件夹
instance-settings-quick-host-invalid = 服务器地址不是有效的主机名
instance-settings-quick-port-invalid = 端口需要是 1 到 65535 的数字
instance-settings-tab-game = 游戏
instance-settings-tab-runtime = 运行时
instance-settings-tab-java = Java
instance-settings-tab-performance = 性能
instance-settings-tab-advanced = 高级

## 账户详情：外观

skin-view-loading = 正在读取外观…
skin-view-aria = 账户外观的立体预览；拖动或方向键旋转，滚轮或加减键缩放，R 复位
skin-view-trial = 试穿中
skin-view-elytra = 鞘翅
skin-view-hint = 拖动旋转 · 滚轮缩放 · 双击复位
skin-view-failed = 没能读到这个账户的外观
skin-view-default-missing = 已安装游戏中没有可用的默认贴图。安装游戏后重新打开详情即可预览。
wardrobe-edit = 编辑外观…
wardrobe-save-current = 保存当前皮肤到皮肤库
wardrobe-default = 恢复默认皮肤
wardrobe-reload = 刷新外观
wardrobe-trial-reset = 还原
wardrobe-trial-wear = 应用
wardrobe-pending = 更改已接受，档案正在同步；若仍显示原来的外观，请刷新。
wardrobe-retry-cape = 重试披风
wardrobe-stale = 档案暂未更新，仍显示上次的外观；可稍后刷新。
wardrobe-library = 皮肤库
wardrobe-loading = 正在读取皮肤库…
wardrobe-add = 添加皮肤
wardrobe-add-drop = 点选或拖入 PNG
wardrobe-rename = 改名…
wardrobe-edit-skin = 编辑…
wardrobe-up = 上移
wardrobe-down = 下移
wardrobe-rename-title = 给皮肤改名
wardrobe-rename-placeholder = 皮肤的名字
wardrobe-edit-title = 编辑外观
wardrobe-editor-current = 账户当前的外观
wardrobe-editor-texture = 纹理
wardrobe-editor-replace = 替换纹理…
wardrobe-editor-arms = 手臂
wardrobe-editor-cape = 披风
wardrobe-no-cape = 不穿
wardrobe-editor-save = 保存到皮肤库
wardrobe-editor-apply = 应用到账户
wardrobe-editor-invalid-image = 无法使用这张 PNG
wardrobe-open-site = 在皮肤站修改
account-look-third-party = 这个账户的皮肤在它的皮肤站上设置。
account-look-error-picture = 皮肤需要是 64×64 或 64×32 的 PNG 图片
account-look-error-network = 连不上 Minecraft 外观服务，请检查网络后重试
account-look-error-protocol = Minecraft 外观服务的回答不符合预期，请稍后重试
account-look-error-refused = Minecraft 外观服务拒绝了这次更改，请稍后重试
account-look-error-rate-limited = 更换得太频繁了，Mojang 要求稍等几分钟再试。
account-look-error-read-only = 这个账户的外观需要在皮肤站修改
account-look-error-cape = 这个账户没有这件披风，请重新读取已拥有的披风
account-look-error-storage = 没能读写本地皮肤库，请检查数据目录后重试
account-look-error-missing = 这张皮肤已经不在本地皮肤库中，请重新选择
account-look-error-order = 皮肤库已经变化，请重新读取后排序
account-make-current = 设为当前账户

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
settings-plugin-write-files = 修改游戏的 { $folder } 文件夹里的路径点文件（先备份，游戏运行时不写）
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

# 设置对话框里的校验提示。
settings-number-whole = { $what }需要填一个整数
settings-number-range = { $what }要在 1 到 { $max } MB 之间
settings-window-size = 窗口需要同时填宽和高，范围 1 到 { $max }
settings-env-name-invalid = 环境变量名“{ $name }”不能为空，也不能含等号、空格或控制字符
settings-control-character = 参数和命令里不能有控制字符
settings-wrapper-unclosed = 包装命令的引号没有闭合，或者没有程序名
settings-quick-play = 直接进入的目标不可用
settings-memory-min-above-max = 最小内存不能超过最大内存
settings-env-line = 第 { $line } 行需要写成 名称=值
settings-concurrency-range = 同时下载数要在 { $min } 到 { $max } 之间
settings-mirror-line = 第 { $line } 行需要写成 官方前缀 => 镜像前缀
settings-mirror-prefixes = 第 { $line } 行的两个前缀都不能为空
# 校验提示里代替字段名的说法，例如「最小内存需要填一个整数」。
settings-field-min-memory = 最小内存
settings-field-max-memory = 最大内存
settings-field-window-width = 窗口宽度
settings-field-window-height = 窗口高度

# Settings 页面触发的操作提示。
settings-java-pick = 选择 Java
settings-java-added = 已添加 Java { $version }
settings-java-not-found = 这里没有找到 Java，请选 java 程序或 JDK 文件夹
settings-java-add-failed = 没能添加 Java
settings-reclaim-failed = 没能检查游戏文件
settings-reclaim-none = 没有多余的游戏文件
settings-cleaned = 已清理 { $size }
settings-clean-files-failed = 没能清理游戏文件
settings-clear-cache-failed = 没能清理缓存
settings-diagnostics-saved = 诊断包已保存到 { $path }
settings-diagnostics-failed = 没能导出诊断包
settings-java-install-started = 开始下载 Java，进度在动态里
settings-java-installed = 已安装 Java { $version }
settings-java-install-failed = 没有装上 Java
settings-save-failed = 没能保存设置
settings-saved-next-launch = 已保存，下次启动时生效
settings-memory-save-failed = 没能保存内存设置
settings-saved-next-download = 已保存，下次下载时生效
settings-mirror-add-failed = 没能添加镜像预设
settings-mirrors-save-failed = 没能保存镜像设置
settings-download-source-save-failed = 没能保存下载源设置
settings-java-roots-save-failed = 没能保存搜索目录
settings-java-save-failed = 没能保存 Java 设置
settings-plugin-save-failed = 没能保存插件设置
settings-plugin-reset-failed = 没能恢复插件默认值
## 世界地图
map-clear-cache = 清除地图缓存
map-cache-cleared = 地图缓存已清除
map-base-seed = 种子预测
map-base-save = 存档
map-overworld = 主世界
map-nether = 下界
map-end = 末地
map-chunks = 区块网格
map-regions = Region 边界
map-go = 前往
map-coordinates-invalid = 输入世界边界内的 X 和 Z 坐标。
map-retry-failed = 重试失败的 { $count } 块
map-version-unsupported = 这一版还不能查
map-seed-needed = 选择世界或输入种子。
map-save-needed = 存档底图只画单人存档；选一个存档世界。
map-save-unreadable = 有个 Region 文件读不了，可能正在写入或已损坏；那一块显示为无数据。
map-unknown-blocks = 有 { $count } 种方块不在颜色表里（多半是模组方块），画成了淡紫色。
map-render-failed = 地图暂时画不了。
map-no-gpu = 没有可用的图形设备，暂时画不了地图。试试更新显卡驱动。
map-provider-stopped = 这个地图来源连续出错，已暂停。重启启动器后可以再试。
map-tile-failed = 有些地图数据没能加载，可以点旁边的按钮重试。
map-seed = 种子
map-seed-placeholder = 数字或文字种子
map-version = 游戏版本
map-seed-empty = 输入一个种子。
map-seed-save-failed = 没能应用种子，可以再试一次。
map-world = 存档世界
map-choose-world = 选择存档世界
map-search-worlds = 搜索存档世界
map-search-versions = 搜索版本
map-layers = 图层
map-pop-out = 在新窗口中打开地图
map-window-title = 世界地图
map-notices = 消息
map-notices-empty = 没有消息。
map-group-structures = 结构
map-estimated = 估计
map-estimated-note = 标「估计」的图层位置是准的，但那里的地形是否允许生成该结构，只能按群系推测，因为 1.18 起这取决于地形高度。
map-structures-hidden = 缩小后结构图标会隐藏。
map-structure-village = 村庄
map-structure-desert-pyramid = 沙漠神殿
map-structure-jungle-temple = 丛林神殿
map-structure-swamp-hut = 沼泽小屋
map-structure-igloo = 雪屋
map-structure-outpost = 掠夺者前哨站
map-structure-monument = 海底神殿
map-structure-mansion = 林地府邸
map-structure-ocean-ruin = 海底废墟
map-structure-shipwreck = 沉船
map-structure-ruined-portal = 废弃传送门
map-structure-ancient-city = 远古城市
map-structure-trail-ruins = 古迹废墟
map-structure-trial-chambers = 试炼密室
map-structure-abandoned-camp = 废弃营地
map-structure-buried-treasure = 埋藏的宝藏
map-structure-mineshaft = 废弃矿井
map-structure-desert-well = 沙漠水井
map-structure-geode = 紫晶洞
map-structure-end-gateway = 末地折跃门
map-structure-stronghold = 要塞
map-structure-fortress = 下界要塞
map-structure-bastion = 堡垒遗迹
map-structure-end-city = 末地城
map-group-world = 世界
map-world-spawn = 出生点
map-save-positions = 存档位置
map-save-spawn = 出生点（存档）
map-save-player = 玩家
map-world-slime = 史莱姆区块
map-fine-layers-hidden = 史莱姆区块要放大到每像素 1 方块才显示。
map-source-seed = 种子预测
map-selection-source = 来源：{ $source }
map-copy-coordinates = 复制坐标
map-selection-close = 关闭
map-group-xaero = Xaero 小地图
map-xaero-waypoints = Xaero 路径点
map-xaero-waypoint = 路径点
map-xaero-waypoint-disabled = 路径点（已停用）
map-xaero-death = 死亡点
map-xaero-unreadable = 这份路径点文件读不了，可能已损坏或来自更新的版本。
map-xaero-suggestion = 发现同名的 Xaero 小地图数据：{ $dir }
map-xaero-link = 关联
map-copy-share = 复制分享串
map-edit = 编辑
map-edit-title = 编辑
map-edit-new-title = 新建路径点
map-edit-locked = 游戏运行时不能改，退出游戏后再改。
map-place = 添加路径点
map-place-cancel = 取消添加
map-placing-hint = 点一下地图，放置新的路径点。
map-delete-title = 删除「{ $name }」？
map-delete-body = 会从 Xaero 的路径点文件里删掉这一个点，原文件会先备份。
map-delete-death-body = 这是一个死亡点，删掉后游戏里就找不回来了。原文件会先备份。
map-edit-failed = 没能保存修改，可以再试一次。
map-edit-running = 游戏正在运行或有别的操作占用这个游戏，退出后再改。
map-edit-conflict = 文件在你编辑期间被改过，已取消写入，请重新选择后再改。
map-edit-stale = 这个路径点已经变了，请重新选择后再改。
map-edit-name-invalid = 名称不能为空，不超过 32 个字符，也不能换行。
map-edit-initials-invalid = 缩写不超过 2 个字符，也不能换行。
map-edit-unlinked = 这个世界还没有关联 Xaero 小地图数据。
map-edit-unsupported = 这个图层不支持编辑。
map-chunks-hidden = 缩放太远时不显示区块线。
