<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 游戏页 · 世界 · 已实现的用户路径

表里每一行都有对应的实现；没做的、范围外的见 [../README.md](../README.md)。

| 操作 | 层 / 组件 | 结果与反馈 | 编号 | 备注 | 实现 |
|---|---|---|---|---|---|
| 导入世界 | L2 次要「导入世界」→ 选 .zip；也可把 .zip 拖进世界页（一次一个） | 识别含 level.dat 的最浅文件夹，解压到 saves，重名自动加序号，不覆盖 | H-WORLD-02 |  | `lumilio-ui/src/instance_panels.rs` |
| 进入世界 | 世界行「进入」 | 启动并直达该世界；1.20 以前的版本置灰并说明 | H-PLAY-07、L-PLAY-02 |  | `lumilio-ui/src/instance_panels.rs` |
| 复制世界 | 世界行 ⋯ 菜单 | 复制到新文件夹（重名自动加序号）→ toast | H-WORLD-07 |  | `lumilio-ui/src/instance_panels.rs` |
| 创建备份 | 世界行 ⋯ 菜单 | 仅这个世界的快照 → 历史·快照可见 | H-WORLD-10、L-HIST-01 |  | `lumilio-ui/src/instance_panels.rs` |
| 导出为 .zip | 世界行 ⋯ 菜单 → 选位置 | 后台打包，toast；不含 session.lock | H-WORLD-09 |  | `lumilio-ui/src/instance_panels.rs` |
| 在访达中显示 | 世界行 ⋯ 菜单 | 打开该世界的目录 | H-WORLD-12 |  | `lumilio-ui/src/instance_panels.rs` |
| 删除世界 | 世界行 🗑 → 警告弹窗 | 删除世界文件夹，写历史 | H-WORLD-08 |  | `lumilio-ui/src/instance_panels.rs` |
| 排序 | L4 分段：最近游玩 / 名称 | 视图状态 | H-WORLD-01 |  | `lumilio-ui/src/instance_panels.rs` |
| 搜索 | L4 搜索框 | 按世界名称或文件夹名过滤 | H-WORLD-01 |  | `lumilio-ui/src/instance_panels.rs` |
