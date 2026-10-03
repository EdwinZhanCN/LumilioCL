<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 游戏库 · 已实现的用户路径

表里每一行都有对应的实现；没做的、范围外的见 [../README.md](../README.md)。

| 操作 | 层 / 组件 | 结果与反馈 | 编号 | 备注 | 实现 |
|---|---|---|---|---|---|
| 删除游戏 | 卡片 ⋯ 菜单 → 警告弹窗 | 先在库里确认，再打开游戏页执行删除 | L-LIB-06 |  | `lumilio-ui/src/pages/live.rs` |
| 菜单：开始游戏 | 卡片 ⋯ 菜单 | 与卡片上的「启动」相同 | H-PLAY-01 |  | `lumilio-ui/src/pages/live.rs` |
| 菜单：打开 | 卡片 ⋯ 菜单 | 游戏页（进历史） | H-INSTANCE-01 |  | `lumilio-ui/src/pages/live.rs` |
| 设为当前游戏 | 卡片 ⋯ 菜单 | 首页、发现页的安装目标和右下角芯片跟着换 | H-NAV-03 |  | `lumilio-ui/src/pages/live.rs` |
| 合集：加入 / 移出 | 卡片 ⋯ 菜单「加入合集…」→ 多选列表 | 勾选即加入或移出，可顺手新建；合集是标签，不移动文件 | ARCH User Collections |  | `lumilio-ui/src/pages/live.rs` |
| 复制游戏 | 卡片 ⋯ 菜单 → 复制弹窗（名称、是否复制存档） | 先打开游戏页再弹窗，之后同游戏页 | L-LIB-05 |  | `lumilio-ui/src/pages/live.rs` |
| 在访达中显示 | 卡片 ⋯ 菜单 | 打开游戏目录 | H-INSTANCE-10 |  | `lumilio-ui/src/pages/live.rs` |
| 导出整合包 | 卡片 ⋯ 菜单 → 导出弹窗 | 先打开游戏页再弹出导出，之后同游戏页 | L-LIB-08、H-INSTANCE-09 |  | `lumilio-ui/src/pages/live.rs` |
| 打开游戏 | 点卡片 | 游戏页（进历史） | H-INSTANCE-01 |  | `lumilio-ui/src/pages/live.rs` |
| 直接启动 | 卡片按键行里的「启动」 | 首页启动时刻接管；不打开游戏页 | H-PLAY-01 |  | `lumilio-ui/src/pages/live.rs` |
| 收藏 / 取消 | 卡片按键行里的星（图标颜色表示状态） | 立即切换；收藏页同步；不打开游戏页 | L-LIB-01 |  | `lumilio-ui/src/pages/live.rs` |
| 合集：改名 / 删除 | 合集节标题 ⋯ 菜单 | 删除先确认，游戏不受影响 | ARCH User Collections |  | `lumilio-ui/src/pages/live.rs` |
| 合集：新建 | 合集标签 L2「新建合集」 | 弹窗取名；名称不重复 | ARCH User Collections |  | `lumilio-ui/src/pages/live.rs` |
| 导入整合包 | L2 次要 → 系统选文件（.mrpack，或 MultiMC/Prism/本启动器备份的 .zip） | 后台导入，进度在动态；完成 toast 并可点“打开” | L-LIB-03、H-INSTALL-04 |  | `lumilio-ui/src/pages/live.rs` |
| 导入其他启动器的游戏 | L2 ⋯ 菜单 → 选文件夹（MultiMC/Prism 实例、.minecraft）→ 有多个时勾选 | 后台导入，复制玩家文件，原文件不动；CurseForge 格式不支持 | L-LIB-03 | ADR 0016 | `lumilio-ui/src/pages/live.rs` |
| 从备份恢复 | L2 ⋯ 菜单 → 选备份 .zip | 后台恢复成新游戏，不覆盖已有的 | — | ADR 0015 | `lumilio-ui/src/pages/live.rs` |
| 打开游戏库文件夹 | L2 ⋯ 菜单 | 在访达中打开所有游戏所在的文件夹 | — |  | `lumilio-ui/src/pages/live.rs` |
| 新建游戏 | L2 主要 → 新建游戏弹窗 | 创建并（可选）立即安装；成功后打开新游戏页 | L-LIB-02、H-INSTALL-01/02 |  | `lumilio-ui/src/pages/live.rs` |
| 搜索 | L3 搜索框 | 按名称过滤；空结果“没有匹配的游戏” | H-NAV-04 |  | `lumilio-ui/src/pages/live.rs` |
| 排序 / 按加载器筛选 | L4 两个下拉（排序、加载器），与发现页同一种控件 | 记在偏好设置里，下次打开还是这样；库里只有一种加载器时不显示加载器下拉 | H-NAV-04 |  | `lumilio-ui/src/pages/live.rs` |
| 拖入整合包 | 把文件拖到页面上 | 同“导入整合包”（.mrpack，或 MultiMC/Prism/本启动器备份的 .zip）；不是的提示一句 | H-INSTALL-04 |  | `lumilio-ui/src/shell.rs` |
