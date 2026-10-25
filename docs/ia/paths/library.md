<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 游戏库 · 已实现的用户路径

表里每一行都有对应的实现；没有的就是没做。

| 操作 | 层 / 组件 | 结果与反馈 | 备注 | 实现 |
|---|---|---|---|---|
| 拖入整合包 | 把文件拖到页面上 | 同“导入整合包”（.mrpack、CurseForge 整合包 .zip，或 MultiMC/Prism/本启动器备份的 .zip）；不是的提示一句 |  | `lumilio-ui/src/features/library/page.rs` |
| 预设：查看 | 预设 tab 左侧列表的一行（名称、共享几项、几个游戏在用；有游戏待决定时亮灯） | 右侧显示这个预设共享的选项和使用它的游戏 |  | `lumilio-ui/src/features/library/presets_view.rs` |
| 预设：重命名 | 预设详情 ⋯ 菜单「重命名…」→ 弹窗 | 名称 1–80 个字、不与其他预设重名；结果 toast 手动关闭 |  | `lumilio-ui/src/features/library/presets_view.rs` |
| 预设：复制 | 预设详情 ⋯ 菜单「复制…」→ 弹窗取名 | 新预设有自己的选项和共享范围，不带走使用它的游戏 |  | `lumilio-ui/src/features/library/presets_view.rs` |
| 预设：删除 | 预设详情 ⋯ 菜单「删除…」→ 警告弹窗 | 只删预设，游戏自己的选项不变；还有游戏在用时不可选 |  | `lumilio-ui/src/features/library/presets_view.rs` |
| 预设：共享范围 | 预设详情头部「共享范围…」→ 弹窗：按组和逐项勾选 | 取消勾选的选项值仍留在预设里，可再勾回；使用它的游戏在运行或有待决定的修改时不能改 |  | `lumilio-ui/src/features/library/presets_view.rs` |
| 预设：打开使用它的游戏 | 预设详情「使用它的游戏」每行的「打开」 | 打开该游戏的 设置 › 选项（有待决定的修改时在最上面），不启动游戏 |  | `lumilio-ui/src/features/library/presets_view.rs` |
| 预设：修改选项 | 预设详情的选项表：开关当场切换，其他选项「编辑」→ 弹窗（滑块和数值 / 分段 / 可搜索列表） | 存进预设，使用它的游戏下次启动时收到；游戏里改过还没决定的同一项，决定时会看到冲突 |  | `lumilio-ui/src/features/library/presets_view.rs` |
| 预设：新建 | 预设 tab 的次要页面操作「新建预设…」→ 弹窗：名称、来源游戏（可搜索）、共享的选项组 | 只读来源游戏的选项文件，不改它；默认不共享画面与性能；建好后在列表中选中，不让任何游戏自动使用 |  | `lumilio-ui/src/features/library/presets_view.rs` |
| 删除游戏 | 卡片 ⋯ 菜单 → 警告弹窗 | 先在库里确认，再打开游戏页执行删除 |  | `lumilio-ui/src/features/library/view.rs` |
| 菜单：开始游戏 | 卡片 ⋯ 菜单 | 与卡片上的「启动」相同 |  | `lumilio-ui/src/features/library/view.rs` |
| 菜单：打开 | 卡片 ⋯ 菜单 | 游戏页（进历史） |  | `lumilio-ui/src/features/library/view.rs` |
| 设为当前游戏 | 卡片 ⋯ 菜单 | 当前游戏换成它：右下角芯片、首页「继续」和发现页的安装目标跟着换 |  | `lumilio-ui/src/features/library/view.rs` |
| 合集：加入 / 移出 | 卡片 ⋯ 菜单「加入合集…」→ 多选列表 | 勾选即加入或移出，可顺手新建；合集是标签，不移动文件 |  | `lumilio-ui/src/features/library/view.rs` |
| 复制游戏 | 卡片 ⋯ 菜单 → 复制弹窗（名称、是否复制存档） | 先打开游戏页再弹窗，之后同游戏页 |  | `lumilio-ui/src/features/library/view.rs` |
| 在访达中显示 | 卡片 ⋯ 菜单 | 打开游戏目录 |  | `lumilio-ui/src/features/library/view.rs` |
| 导出整合包 | 卡片 ⋯ 菜单 → 导出弹窗 | 先打开游戏页再弹出导出，之后同游戏页 |  | `lumilio-ui/src/features/library/view.rs` |
| 打开游戏 | 点卡片 | 游戏页（进历史） |  | `lumilio-ui/src/features/library/view.rs` |
| 直接启动 | 卡片按键行里的「启动」 | 先设为当前游戏（芯片和「继续」跟着换），再由首页启动时刻接管；不打开游戏页 | global-context D4 | `lumilio-ui/src/features/library/view.rs` |
| 收藏 / 取消 | 卡片按键行里的星（图标颜色表示状态） | 立即切换；收藏页同步；不打开游戏页 |  | `lumilio-ui/src/features/library/view.rs` |
| 合集：改名 / 删除 | 合集节标题 ⋯ 菜单 | 删除先确认，游戏不受影响 |  | `lumilio-ui/src/features/library/view.rs` |
| 导入其他启动器的游戏 | L2 ⋯ 菜单 → 选文件夹（MultiMC/Prism 实例、.minecraft）→ 有多个时勾选 | 后台导入，复制玩家文件，原文件不动；CurseForge 格式不支持 | ADR 0016 | `lumilio-ui/src/features/library/view.rs` |
| 从备份恢复 | L2 ⋯ 菜单 → 选备份 .zip | 后台恢复成新游戏，不覆盖已有的 | ADR 0015 | `lumilio-ui/src/features/library/view.rs` |
| 打开游戏库文件夹 | L2 ⋯ 菜单 | 在访达中打开所有游戏所在的文件夹 |  | `lumilio-ui/src/features/library/view.rs` |
| 新建游戏 | L2 主要 → 新建游戏弹窗 | 创建并（可选）立即安装；成功后打开新游戏页 |  | `lumilio-ui/src/features/library/view.rs` |
| 搜索 | L3 搜索框 | 按名称过滤；空结果“没有匹配的游戏” |  | `lumilio-ui/src/features/library/view.rs` |
| 预设：搜索选项 | 预设 tab 的 L3 搜索框 | 按名称或值过滤右侧的选项；没有匹配时说一句 |  | `lumilio-ui/src/features/library/view.rs` |
| 合集：新建 | 合集标签 L2「新建合集」 | 弹窗取名；名称不重复 |  | `lumilio-ui/src/features/library/view.rs` |
| 导入整合包 | L2 次要 → 系统选文件（.mrpack、CurseForge 整合包 .zip，或 MultiMC/Prism/本启动器备份的 .zip） | 后台导入，进度在动态；完成后 toast，并打开新游戏的页面、设为当前游戏（加载器筛选不会把它藏起来）；CurseForge 整合包的文件由 CurseForge 来源解析，作者只允许在网站下载的先按 SHA-1 到 Modrinth 找，找不到的列在游戏的内容页 | CurseForge 整合包需要开着 CurseForge 来源 | `lumilio-ui/src/features/library/view.rs` |
| 排序 / 按加载器筛选 | L4 两个下拉（排序、加载器），与发现页同一种控件 | 记在偏好设置里，下次打开还是这样；库里只有一种加载器时不显示加载器下拉 |  | `lumilio-ui/src/features/library/view.rs` |
