# 信息架构（IA）

**IA 不再手写。** 页面上有哪些用户路径，由代码里的 `// ia[page]: …` 注释声明，`cargo run -p lumilio-docgen -- ia` 生成到 [paths/](paths/README.md)（ADR 0019，流程见 `.agents/skills/lumilio-ia-paths/`）。生成表里有一行就等于做了；没有的就是没做。

页面的版面和视觉规则在 [../design-language.md](../design-language.md)，跨页面复用的交互模式（P-…）在 [../design-patterns.md](../design-patterns.md)，业务契约和流程编号（H-… / L-… / AC-…）在 [../workflows/](../workflows/README.md)，产品骨架在 [ARCH.md](../../ARCH.md)。

## 没做的 / 范围外

这里只列有意不做或还没做的，做了就从这里删，并在代码里加 `// ia` 注释。

| 范围 | 没做的 | 原因 / 前提 |
|---|---|---|
| 账户 | 离线皮肤（H-ACC-11） | ADR 0018 推迟：需要第三方认证范围与 authlib-injector |
| 账户 | 第三方认证（H-ACC-03/04/05） | 同上 |
| 设置 | 语言（只有简体中文） | — |
| 设置 | 检查更新（启动器自动更新） | 需要先写 ADR |
| 发现 | CurseForge 等其他来源；下载世界 | 需要来源 ADR；CurseForge 还要 API 密钥 |
| 游戏库 | 从 CurseForge 导入 | 需要它的 API 和密钥 |
| 游戏库 | 自定义实例图标 | 像素封面优先 |
| 游戏库 | 外部游戏目录登记（H-NAV-05/06/07） | App State 首轮不支持 |
| 游戏页·内容 | 固定版本；拖入文件；“全部更新”弹窗里逐项取消勾选；识别结果缓存 | 固定版本要在实例数据里记录 |
| 游戏页·内容 | 内容页自动补依赖；整合包内容单独更新时的提示（L-CONT-03） | 待决策；后者等整合包更新能力 |
| 游戏页·世界 | 世界属性/玩家数据编辑（H-WORLD-05/06）、数据包管理（H-WORLD-11）、在线下载世界（H-WORLD-03）、多人服务器列表 | 需要范围 ADR |
| 游戏页·诊断 | 文件编辑、NBT 编辑（H-EXTRA-06）；线程 dump | — |
| 整合包 | 整合包更新（识别包管理文件） | 还没排期 |
