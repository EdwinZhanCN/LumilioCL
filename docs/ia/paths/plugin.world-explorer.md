<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 插件 · 世界地图 · 已实现的用户路径

表里每一行都有对应的实现；没有的就是没做。

| 操作 | 层 / 组件 | 结果与反馈 | 备注 | 实现 |
|---|---|---|---|---|
| 打开地图 | 游戏页「插件」标签内的地图 | 宿主显示世界选择与地图；没有存档时可输入手动种子 | 插件默认启用，关闭后入口消失 | `lumilio-plugin-world-explorer/src/lib.rs` |
| 显示存档位置 | 图层浮层 ·「存档位置」开关 | 画出 level.dat 里的实际出生点和单人玩家最后的位置，只在所在维度显示；点选可看坐标并复制 | 默认打开；只对单人存档出现，此时种子估算的出生点不再显示 | `lumilio-plugin-world-explorer/src/save/positions.rs` |
| 退出游戏后更新存档底图 | 地图视口 · 游戏结束时自动 | 重新取可见瓦片；只有 Region 文件长度或修改时间变了的块及其上层合成块重画，其余用缓存 | 缓存在 profiles/<实例>/map-cache，可在设置 › 存储清除 | `lumilio-ui/src/instance_detail/data.rs` |
| 新建路径点 | 地图左上角「添加路径点」→ 点一下地图 | 打开新建对话框，位置取点击处；游戏运行中禁用并说明原因 |  | `lumilio-ui/src/world_explorer/edit.rs` |
| 打开图层 | 地图视口右下角 ·「图层」 | 打开次要图层浮层；Escape 或点击外部关闭并返回焦点 |  | `lumilio-ui/src/world_explorer/layers.rs` |
| 显示区块网格 | 图层浮层 · 区块网格开关 | 即刻开关；每像素超过 4 方块时隐藏细线，并在开关旁说明 |  | `lumilio-ui/src/world_explorer/layers.rs` |
| 显示 Region 边界 | 图层浮层 · Region 边界开关 | 即刻开关 512 方块边界；不改变底图或相机 |  | `lumilio-ui/src/world_explorer/layers.rs` |
| 显示或隐藏结构图层 | 图层浮层 · 按种类分组的开关 | 即刻开关，图标按世界版本与维度出现；每像素超过 16 方块时隐藏并说明；1.18 起沙漠神殿、丛林神殿、林地府邸标「估计」 |  | `lumilio-ui/src/world_explorer/layers.rs` |
| 查看存档底图 | 底图分段 ·「存档」 | 从单人存档的 Region 文件画地表（按高度明暗、按群系着色）；没生成完的区块显示无数据；颜色表里没有的方块画成淡紫色并在消息里写明种数 | 粗缩放由宿主用细一级合成 | `lumilio-ui/src/world_explorer/mod.rs` |
| 平移与缩放 | 地图视口 · 拖动 / 滚轮 / + − 与方向键 | 锚点缩放；过期世代与旧视口结果不进入当前帧 |  | `lumilio-ui/src/world_explorer/mod.rs` |
| 查看地图消息 | 地图左下角 ·「消息」键与小红点 | 有失败、缺少的输入或可关联的 Xaero 数据时键上亮一个点（不带数字）；点开列出消息、重试键与关联键；Escape 或点击外部关闭 |  | `lumilio-ui/src/world_explorer/notices.rs` |
| 点选地图对象 | 地图右下角信息卡 · 种类、坐标、来源与「复制坐标」 | 点图标选中并显示；点空白处或「×」取消；复制 `x z`；估计的位置标「估计」 |  | `lumilio-ui/src/world_explorer/select.rs` |
| 复制路径点分享串 | 信息卡 ·「复制分享串」 | 复制 Xaero 的 xaero-waypoint: 格式，可在游戏里导入；只在路径点上出现 |  | `lumilio-ui/src/world_explorer/select.rs` |
| 输入手动种子 | 地图顶部 · 种子输入框 | 数字或文字种子，Enter 或失焦应用；错误留在输入框旁；保存到该实例 |  | `lumilio-ui/src/world_explorer/toolbar.rs` |
| 选择种子版本 | 种子旁 · 可搜索版本下拉 | 默认世界的受支持版本，否则最新的受支持版本（26.3）；确认后应用输入种子，不自动降级存档 |  | `lumilio-ui/src/world_explorer/toolbar.rs` |
| 选择世界 | 地图顶部 · 存档下拉 | 只列该实例存档；无存档时隐藏；选择后同步种子与版本，取消上一世界请求 |  | `lumilio-ui/src/world_explorer/toolbar.rs` |
| 切换维度 | 地图左上角 · 维度标签 | 保留相机；清除旧维度帧和瓦片，取消旧请求 |  | `lumilio-ui/src/world_explorer/toolbar.rs` |
| 选择底图 | 地图右上角 · 底图标签 | 单选；保留相机与维度，清除旧帧并取消旧底图请求 |  | `lumilio-ui/src/world_explorer/toolbar.rs` |
| 打开坐标跳转 | 地图左下角 · 光标坐标读数 | 点读数换成 X、Z 输入框并聚焦 X；读数随鼠标在地图上移动 |  | `lumilio-ui/src/world_explorer/toolbar.rs` |
| 前往坐标 | 地图左下角 · X、Z 两个输入框与「前往」键 | 点「前往」或在任一框按 Enter 把视图中心移到该点并收回读数；Esc 取消；X 或 Z 无法读取时在框上说明，视图不动 |  | `lumilio-ui/src/world_explorer/toolbar.rs` |
| 重试失败瓦片 | 地图消息菜单 ·「重试失败的 N 块」 | 一次重新派发全部失败块；没有失败时隐藏；暂停的来源重启后恢复 |  | `lumilio-ui/src/world_explorer/toolbar.rs` |
| 在新窗口打开地图 | 地图视口右下角 · 图层键左侧的全屏图标键 | 在独立窗口打开同一世界、维度、位置与图层，原视图不变；已在独立窗口里的地图不显示此键 |  | `lumilio-ui/src/world_explorer/window.rs` |
| 关联 Xaero 路径点 | 地图消息菜单 · 「发现同名的 Xaero 小地图数据」与「关联」 | 只在存档有同名目录且未关联时出现；确认后保存，路径点图层随即可用；不会自动关联 |  | `lumilio-ui/src/world_explorer/xaero.rs` |
