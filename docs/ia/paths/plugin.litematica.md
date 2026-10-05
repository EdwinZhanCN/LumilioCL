<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 插件 · Litematica 投影 · 已实现的用户路径

表里每一行都有对应的实现；没有的就是没做。

| 操作 | 层 / 组件 | 结果与反馈 | 备注 | 实现 |
|---|---|---|---|---|
| 打开一份投影 | 列表行（缩略图、名称、作者与尺寸、方块数） | 进入详情：事实、材料清单；读不了的文件只在自己那一行写「读不了」，不影响列表 |  | `lumilio-plugin-litematica/src/tab.rs` |
| 返回列表 | 详情页次要键 | 回到投影列表 |  | `lumilio-plugin-litematica/src/tab.rs` |
| 3D 预览 | 详情页的「3D 预览」卡片和键 | 打开单独的预览窗口，用游戏自己的贴图画出整个投影，可旋转缩放；游戏没装时没有贴图；Linux 上没有这个键 | 宿主负责窗口，插件只在视图里写「这里有个模型」 | `lumilio-plugin-litematica/src/tab.rs` |
| 在文件夹中显示 | 详情页次要键 | 在访达或资源管理器里选中这份投影文件 |  | `lumilio-plugin-litematica/src/tab.rs` |
| 导出材料清单（CSV） | 详情页主要键 | 弹出保存对话框，选了位置才写入；按数量从多到少，表头为「方块,数量」 | 文件带 UTF-8 标记，表格软件直接读中文 | `lumilio-plugin-litematica/src/tab.rs` |
