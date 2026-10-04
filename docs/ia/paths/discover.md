<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 发现 · 已实现的用户路径

表里每一行都有对应的实现；没有的就是没做。

| 操作 | 层 / 组件 | 结果与反馈 | 备注 | 实现 |
|---|---|---|---|---|
| 锁定目标（从游戏页进入） | 游戏页内容标签「浏览 Mod / 资源包 / 光影」 | 发现页进入「为这个游戏浏览」：页头写明游戏，类型预选，版本和加载器筛选锁定；安装目标换成那个游戏（右下角芯片显示）；不改变当前游戏 |  | `lumilio-app/src/live/instance.rs` |
| 依赖提示 | 安装 Mod 前的弹窗 | 列出目标游戏还缺的必需依赖（默认勾选；没有适合版本的标明且不能勾；「只安装它」跳过）；也列可选依赖（默认不勾）并警告与已装 Mod 不兼容；依赖先装，每个是动态里的一条任务 | 仅 Mod | `lumilio-ui/src/dependency_prompt.rs` |
| 安装（最新兼容版本） | 结果行「安装」 | 后台任务，toast“开始安装”，完成 toast，期间按钮显示「安装中…」；Mod 先过依赖提示 |  | `lumilio-ui/src/pages/live/discover/card.rs` |
| 已安装状态 | 结果行「已安装」/「更新」 | 目标游戏已有的（按 Modrinth 识别）显示「已安装」；有更新显示「更新」，点了用新版本替换旧文件；详情页主按钮同样变化 | 只认 Modrinth 认得的文件 | `lumilio-ui/src/pages/live/discover/card.rs` |
| 打开项目详情 | 点结果行 | 详情页（进历史） |  | `lumilio-ui/src/pages/live/discover/card.rs` |
| 结果行右键菜单 | 在 Modrinth 中打开 / 复制链接 | 浏览器打开项目页，或把项目页地址放进剪贴板并提示 |  | `lumilio-ui/src/pages/live/discover/card.rs` |
| 分页 | 列表上方的页码键 | 翻页并回到列表顶部的第一行 |  | `lumilio-ui/src/pages/live/discover/mod.rs` |
| 连不上 | 搜索失败的整页提示 | 说明连不上 Modrinth，附技术详情；改任何条件会再试一次 |  | `lumilio-ui/src/pages/live/discover/mod.rs` |
| 分类 | L3 标签：整合包 / Mod / 资源包 / 光影（游戏页进入时去掉整合包，原版游戏再去掉 Mod） | 切换搜索的项目类型并重新搜索；类型一换，排序回到相关度、搜索词清空 |  | `lumilio-ui/src/pages/live/discover/mod.rs` |
| 搜索 | L3 搜索框（回车确认，有清除键） | 按关键词搜索 Modrinth |  | `lumilio-ui/src/pages/live/discover/mod.rs` |
| 排序 / 显示数量 | L4 两个下拉：相关度、下载量、关注数、发布时间、更新时间；每页 5 / 10 / 15 / 20 / 50 / 100 | 重新搜索；按发布时间排序时卡片显示发布时间，其余显示更新时间 |  | `lumilio-ui/src/pages/live/discover/mod.rs` |
| 筛选 | 右侧丝印编号筛选栏，按类型分区、可折叠：游戏版本（可搜索，可显示全部版本）、加载器（常用在前，「显示更多」）、类别、运行环境、许可证、高级排除；每个选项点一下为「要」，点 ⊘ 为「不要」 | 重新搜索；游戏提供的版本和加载器带锁 |  | `lumilio-ui/src/pages/live/discover/sidebar.rs` |
| 高级排除 | 筛选栏「高级排除」：光敏性触发、AI、广告、遥测等 | 选择会被记住；第一次选「光敏性触发」弹出提示，可选不再提示 |  | `lumilio-ui/src/pages/live/discover/sidebar.rs` |
| 隐藏已安装 | 筛选栏顶部开关（整合包页，或从游戏页进入时） | 搜索时排除已有的整合包 / 目标游戏已装的项目；整合包页的选择会被记住 |  | `lumilio-ui/src/pages/live/discover/sidebar.rs` |
| 光敏性提示 | 第一次选「高级排除 · 光敏性触发」时的弹窗 | 说明这个筛选只依据作者的声明，不能保证安全；可选「知道了，不再提示」 |  | `lumilio-ui/src/photosensitivity.rs` |
| 画廊 | 详情页「画廊」页签（有图才出现）：卡片带标题、说明、日期 | 点一张在页内放大，可前后翻、在浏览器打开、关闭 |  | `lumilio-ui/src/project_detail/gallery.rs` |
| 安装整合包 | 详情页主按钮「安装为新游戏」 | 新建游戏，完成后 toast 可“打开” |  | `lumilio-ui/src/project_detail/header.rs` |
| 详情页安装按钮 | 主按钮：安装 / 安装中 / 更新 / 切换版本（已装且在介绍页）/ 已安装（已装且在版本页） | 目标游戏已有的才出现后几种；「切换版本」带你去版本页 |  | `lumilio-ui/src/project_detail/header.rs` |
| 详情页右键菜单 | 在页头上右键：安装 / 在 Modrinth 中打开 / 复制链接 | 与「⋯」菜单同效 |  | `lumilio-ui/src/project_detail/header.rs` |
| 安装指定版本 | 详情页版本行「安装」/「切换」/「已安装」 | 后台任务；已装的这一版显示已安装，已装别的版本显示切换（换成这一版）；与目标游戏不兼容的版本禁用 |  | `lumilio-ui/src/project_detail/versions.rs` |
| 版本筛选 | 版本页签顶部一行：发布渠道（正式 / 测试 / 早期）、加载器、添加游戏版本（可搜索），已选的游戏版本是可点掉的标签 | 列表即时过滤；从游戏页进入时默认筛到这个游戏的版本和加载器 |  | `lumilio-ui/src/project_detail/versions.rs` |
| 另存为文件 | 详情页版本行「另存为」图标 | 选位置下载，任何类型，包括整合包文件 |  | `lumilio-ui/src/project_detail/versions.rs` |
