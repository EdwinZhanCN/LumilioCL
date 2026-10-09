<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 插件 · 世界地图 · 已实现的用户路径

表里每一行都有对应的实现；没有的就是没做。

| 操作 | 层 / 组件 | 结果与反馈 | 备注 | 实现 |
|---|---|---|---|---|
| 打开地图 | 游戏页「插件」标签内的地图 | 宿主显示世界选择与地图；没有存档时可输入手动种子 | 插件默认启用，关闭后入口消失 | `lumilio-plugin-world-explorer/src/lib.rs` |
| 打开图层 | 地图视口右下角 ·「图层」 | 打开次要图层浮层；Escape 或点击外部关闭并返回焦点 |  | `lumilio-ui/src/world_explorer/layers.rs` |
| 显示区块网格 | 图层浮层 · 区块网格开关 | 即刻开关；每像素超过 4 方块时隐藏细线，并在开关旁说明 |  | `lumilio-ui/src/world_explorer/layers.rs` |
| 显示 Region 边界 | 图层浮层 · Region 边界开关 | 即刻开关 512 方块边界；不改变底图或相机 |  | `lumilio-ui/src/world_explorer/layers.rs` |
| 平移与缩放 | 地图视口 · 拖动 / 滚轮 / + − 与方向键 | 锚点缩放；过期世代与旧视口结果不进入当前帧 |  | `lumilio-ui/src/world_explorer/mod.rs` |
| 输入手动种子 | 地图顶部 · 种子输入框 | 数字或文字种子，Enter 或失焦应用；错误留在输入框旁；保存到该实例 |  | `lumilio-ui/src/world_explorer/toolbar.rs` |
| 选择种子版本 | 种子旁 · 可搜索版本下拉 | 默认世界的受支持版本，否则 1.21.4；确认后应用输入种子，不自动降级存档 |  | `lumilio-ui/src/world_explorer/toolbar.rs` |
| 选择世界 | 地图顶部 · 存档下拉 | 只列该实例存档；无存档时隐藏；选择后同步种子与版本，取消上一世界请求 |  | `lumilio-ui/src/world_explorer/toolbar.rs` |
| 切换维度 | 地图左上角 · 维度分段 | 保留相机；清除旧维度帧和瓦片，取消旧请求 |  | `lumilio-ui/src/world_explorer/toolbar.rs` |
| 选择底图 | 地图左上角 · 底图分段 | 单选；保留相机与维度，清除旧帧并取消旧底图请求 |  | `lumilio-ui/src/world_explorer/toolbar.rs` |
| 前往坐标 | 地图右上角 · X、Z 两个输入框与「前往」键 | 点「前往」或在任一框按 Enter 把视图中心移到该点；X 或 Z 无法读取时在框下说明，视图不动 |  | `lumilio-ui/src/world_explorer/toolbar.rs` |
| 重试失败瓦片 | 地图左下角状态条 ·「重试失败的 N 块」 | 一次重新派发全部失败块；没有失败时隐藏；暂停的来源重启后恢复 |  | `lumilio-ui/src/world_explorer/toolbar.rs` |
