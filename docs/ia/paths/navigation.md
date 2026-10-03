<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 全局导航 · 已实现的用户路径

表里每一行都有对应的实现；没做的路径写在同目录手写的页面文档里。

| 操作 | 层 / 组件 | 结果与反馈 | 编号 | 备注 | 实现 |
|---|---|---|---|---|---|
| 点地标 | 中段胶囊按钮 | 打开该页并清空“前进” | — |  | `lumilio-ui/src/navigation.rs` |
| 后退 / 前进 | 左段的两个图标按钮 | 回到上/下一个位置；位置 = 地标页、游戏页、项目详情；标签/筛选/滚动不进历史 | — |  | `lumilio-ui/src/navigation.rs` |
| 没有账户时 | 芯片显示“添加账户” | 打开账户页的添加离线账户弹窗 | H-ACC-02 |  | `lumilio-ui/src/navigation.rs` |
| 切换账户 | 右段 Popover 列表：头像、名字、类型（离线/Microsoft）；单选；底部“管理账户…” → 账户页 | 之后的启动使用该身份 | H-ACC-06 |  | `lumilio-ui/src/navigation.rs` |
| 没有游戏时点芯片 | 右段文字按钮“还没有游戏” | 打开游戏库 | — |  | `lumilio-ui/src/navigation.rs` |
| 切换当前游戏 | 右段 Popover 列表：封面、名称、版本 · 加载器；当前项高亮；游戏多时顶部出现搜索 | 之后的“开始游戏”和发现页安装都指向它；已开始的操作不受影响；保存在 settings.json，重启后保持，被删除时回落到剩余第一个 | H-NAV-03 |  | `lumilio-ui/src/navigation.rs` |
| 后退 / 前进快捷键 | ⌘[ / ⌘]；游戏页、项目详情里 Esc | 同后退 / 前进 | — |  | `lumilio-ui/src/shell.rs` |
