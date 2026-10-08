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
account-menu-skin = 皮肤…
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
