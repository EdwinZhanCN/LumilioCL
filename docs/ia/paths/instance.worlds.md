<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 游戏页 · 世界 · 已实现的用户路径

表里每一行都有对应的实现；没有的就是没做。

| 操作 | 层 / 组件 | 结果与反馈 | 备注 | 实现 |
|---|---|---|---|---|
| 添加服务器 | L2 次要「添加服务器」→ 弹窗（名称、地址） | 追加到服务器列表，游戏里立刻可见；游戏运行时不可改 |  | `lumilio-ui/src/instance_detail/panels/servers.rs` |
| 刷新状态 | L2 次要「刷新状态」 | 逐个检查服务器：在线人数、延迟、版本和 MOTD；连不上显示“无法连接”（5 秒超时） |  | `lumilio-ui/src/instance_detail/panels/servers.rs` |
| 进入服务器 | 服务器行「进入」 | 启动并直连该服务器；1.20 以前的版本置灰并说明 |  | `lumilio-ui/src/instance_detail/panels/servers.rs` |
| 编辑服务器 | 服务器行 ⋯ 菜单 → 弹窗 | 改名称或地址，保留游戏记下的图标等其他信息 |  | `lumilio-ui/src/instance_detail/panels/servers.rs` |
| 排序服务器 | 服务器行 ⋯ 菜单「上移 / 下移」 | 调整在游戏里的显示顺序 |  | `lumilio-ui/src/instance_detail/panels/servers.rs` |
| 复制服务器地址 | 服务器行 ⋯ 菜单 | 复制到剪贴板，toast“已复制地址” |  | `lumilio-ui/src/instance_detail/panels/servers.rs` |
| 删除服务器 | 服务器行 🗑 → 警告弹窗 | 只从列表移除，不影响服务器本身 |  | `lumilio-ui/src/instance_detail/panels/servers.rs` |
| 切换世界 / 服务器 | L4 分段：世界 / 服务器 | 视图状态；第一次进入服务器时读取列表并检查各服务器状态 |  | `lumilio-ui/src/instance_detail/panels/worlds.rs` |
| 导入世界 | L2 次要「导入世界」→ 选 .zip；也可把 .zip 拖进世界页（一次一个） | 识别含 level.dat 的最浅文件夹，解压到 saves，重名自动加序号，不覆盖 |  | `lumilio-ui/src/instance_detail/panels/worlds.rs` |
| 进入世界 | 世界行「进入」 | 启动并直达该世界；1.20 以前的版本置灰并说明 |  | `lumilio-ui/src/instance_detail/panels/worlds.rs` |
| 复制世界 | 世界行 ⋯ 菜单 | 复制到新文件夹（重名自动加序号）→ toast |  | `lumilio-ui/src/instance_detail/panels/worlds.rs` |
| 创建备份 | 世界行 ⋯ 菜单 | 仅这个世界的快照 → 历史·快照可见 |  | `lumilio-ui/src/instance_detail/panels/worlds.rs` |
| 导出为 .zip | 世界行 ⋯ 菜单 → 选位置 | 后台打包，toast；不含 session.lock |  | `lumilio-ui/src/instance_detail/panels/worlds.rs` |
| 在访达中显示 | 世界行 ⋯ 菜单 | 打开该世界的目录 |  | `lumilio-ui/src/instance_detail/panels/worlds.rs` |
| 删除世界 | 世界行 🗑 → 警告弹窗 | 删除世界文件夹，写历史 |  | `lumilio-ui/src/instance_detail/panels/worlds.rs` |
| 排序 | L4 分段：最近游玩 / 名称 | 视图状态 |  | `lumilio-ui/src/instance_detail/panels/worlds.rs` |
| 搜索 | L4 搜索框 | 按世界名称或文件夹名过滤 |  | `lumilio-ui/src/instance_detail/panels/worlds.rs` |
