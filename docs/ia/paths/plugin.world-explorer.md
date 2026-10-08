<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 插件 · 世界地图 · 已实现的用户路径

表里每一行都有对应的实现；没有的就是没做。

| 操作 | 层 / 组件 | 结果与反馈 | 备注 | 实现 |
|---|---|---|---|---|
| 打开地图 | 游戏页「插件」标签内的地图 | 宿主显示世界选择与地图；没有存档时可输入手动种子 | 插件默认启用，关闭后入口消失 | `lumilio-plugin-world-explorer/src/lib.rs` |
| 输入手动种子 | 地图顶部 ·「输入种子…」 | 数字或文字种子，明确选择支持的版本；保存到该实例的 launcher.db |  | `lumilio-ui/src/world_explorer/mod.rs` |
| 切换维度 | 地图工具栏 · 维度分段 | 保留相机；清除旧维度瓦片并取消旧请求 |  | `lumilio-ui/src/world_explorer/mod.rs` |
| 选择世界 | 地图顶部 · 存档名称 | 使用该世界的种子与版本，取消上一世界请求 |  | `lumilio-ui/src/world_explorer/mod.rs` |
| 选择底图 | 地图工具栏 · 底图按键 | 单选；保留相机与维度，取消旧底图请求 |  | `lumilio-ui/src/world_explorer/mod.rs` |
| 显示区块与 Region 网格 | 地图工具栏 · 网格开关 | 只改变宿主 Overlay；粗缩放自动隐藏区块细线 |  | `lumilio-ui/src/world_explorer/mod.rs` |
| 跳到坐标 | 地图工具栏 ·「跳到坐标…」 | 校验 X/Z；移动地图中心 |  | `lumilio-ui/src/world_explorer/mod.rs` |
| 重试失败瓦片 | 地图状态行 · 瓦片坐标按键 | 重新派发该块；连续五次故障停用 Provider 到重启 |  | `lumilio-ui/src/world_explorer/mod.rs` |
| 平移与缩放 | 地图视口 · 拖动 / 滚轮 / + − 与方向键 | 锚点缩放；过期世代与旧视口结果不进入当前帧 |  | `lumilio-ui/src/world_explorer/mod.rs` |
