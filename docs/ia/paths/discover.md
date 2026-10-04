<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 发现 · 已实现的用户路径

表里每一行都有对应的实现；没做的、范围外的见 [../README.md](../README.md)。

| 操作 | 层 / 组件 | 结果与反馈 | 编号 | 备注 | 实现 |
|---|---|---|---|---|---|
| 锁定目标（从游戏页进入） | 游戏页内容标签「浏览 Mod / 资源包 / 光影」 | 安装目标换成那个游戏（右下角芯片显示），类型预选；不改变当前游戏 | — |  | `lumilio-app/src/live/instance.rs` |
| 依赖提示 | 安装 Mod 前的弹窗 | 列出目标游戏还缺的必需依赖（默认勾选；没有适合版本的标明且不能勾；「只安装它」跳过）；也列可选依赖（默认不勾）并警告与已装 Mod 不兼容；依赖先装，每个是动态里的一条任务 | H-DISC-03 | 仅 Mod | `lumilio-ui/src/dependency_prompt.rs` |
| 安装（最新兼容版本） | 结果行「安装」 | 后台任务，toast“开始安装”，完成 toast；Mod 先过依赖提示 | H-DISC-04 |  | `lumilio-ui/src/pages/live/discover.rs` |
| 已安装状态 | 结果行「已安装」/「更新」 | 目标游戏已有的（按 Modrinth 识别）显示「已安装」；有更新显示「更新」，点了用新版本替换旧文件；详情页主按钮同样变化 | — | 只认 Modrinth 认得的文件 | `lumilio-ui/src/pages/live/discover.rs` |
| 打开项目详情 | 点结果行 | 详情页（进历史） | H-DISC-02 |  | `lumilio-ui/src/pages/live/discover.rs` |
| 分页 | 列表上方的页码键 | 翻页并回到列表顶部的第一行 | H-DISC-01 |  | `lumilio-ui/src/pages/live/discover.rs` |
| 筛选 | 右侧丝印编号筛选栏：游戏版本、加载器、类别；清除筛选 | 重新搜索；加载器与类别是带 LED 的选项列表 | H-DISC-01 |  | `lumilio-ui/src/pages/live/discover.rs` |
| 分类 | L3 标签：整合包 / Mod / 资源包 / 光影 | 切换搜索的项目类型并重新搜索 | H-DISC-01 |  | `lumilio-ui/src/pages/live/discover.rs` |
| 搜索 | L3 搜索框（回车确认） | 按关键词搜索 Modrinth | H-DISC-01 |  | `lumilio-ui/src/pages/live/discover.rs` |
| 排序 / 显示数量 | L4 两个下拉 | 重新搜索 | H-DISC-01 |  | `lumilio-ui/src/pages/live/discover.rs` |
| 安装整合包 | 详情页主按钮「安装为新游戏」 | 新建游戏，完成后 toast 可“打开” | H-INSTALL-03 |  | `lumilio-ui/src/project_detail.rs` |
| 另存为文件 | 详情页版本行「另存为…」 | 选位置下载，任何类型，包括整合包文件 | H-DISC-05 |  | `lumilio-ui/src/project_detail.rs` |
| 安装指定版本 | 详情页版本标签行内「安装」 | 后台任务；与目标游戏不兼容的版本禁用 | H-DISC-03/04 |  | `lumilio-ui/src/project_detail.rs` |
