<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 游戏页（整体） · 已实现的用户路径

表里每一行都有对应的实现；没有的就是没做。

| 操作 | 层 / 组件 | 结果与反馈 | 备注 | 实现 |
|---|---|---|---|---|
| 出现「投影」标签 | 游戏页「插件」标签内的左侧列表 | 装了 Litematica 或 schematics 文件夹里已有文件时出现；在设置 → 插件里关掉就消失 | 空的 schematics 文件夹不算（插件只能列出文件） | `lumilio-plugin-litematica/src/lib.rs` |
| 结束游戏 | 本启动器启动的游戏运行时，页头主按钮变「结束游戏」 | 终止进程，会话写入历史 |  | `lumilio-ui/src/instance_detail/actions.rs` |
| 开始游戏 | 页头主按钮「启动游戏」 | 首页启动时刻接管 |  | `lumilio-ui/src/instance_detail/actions.rs` |
| 安装游戏文件 | 页头 ⋯ 菜单（游戏还没装好时才有） | 后台任务，动态可见；完成 toast |  | `lumilio-ui/src/instance_detail/actions.rs` |
| 设为当前游戏 | 页头 ⋯ 菜单 | 导航右段芯片跟着换；之后的启动指向它 |  | `lumilio-ui/src/instance_detail/actions.rs` |
| 修复游戏文件 | 页头 ⋯ 菜单 | 核对并补齐/重下损坏文件，后台任务，动态可见 | 运行中或没装好时禁用 | `lumilio-ui/src/instance_detail/actions.rs` |
| 创建快照 | 页头 ⋯ 菜单 → 弹窗（备注可空；范围＝全部或某个世界） | 后台创建，历史·快照可见 |  | `lumilio-ui/src/instance_detail/actions.rs` |
| 在访达中显示 | 页头 ⋯ 菜单 | 打开游戏目录 |  | `lumilio-ui/src/instance_detail/actions.rs` |
| 复制游戏 | 页头 ⋯ 菜单 → 弹窗（新名称、是否复制存档） | 后台复制成独立副本；不复制历史、快照和游玩时间 |  | `lumilio-ui/src/instance_detail/actions.rs` |
| 完整备份 | 页头 ⋯ 菜单 → 选位置 | 后台打成一个 zip（含存档，不含日志），toast；之后可在游戏库「从备份恢复」 | ADR 0015；运行中禁用 | `lumilio-ui/src/instance_detail/actions.rs` |
| 导出整合包 | 页头 ⋯ 菜单 → 导出弹窗（格式、勾选文件） | 后台导出，可取消 |  | `lumilio-ui/src/instance_detail/actions.rs` |
| 删除游戏 | 页头 ⋯ 菜单 → 警告弹窗 | 删除后从历史中移除并后退 |  | `lumilio-ui/src/instance_detail/actions.rs` |
| 打开插件标签 | 游戏页「插件」标签内的左侧列表 | 读取并显示插件给的内容；插件被关闭或出错时标签消失，回到内置标签 | 插件只描述内容，版式由启动器统一 | `lumilio-ui/src/instance_detail/plugin_tabs.rs` |
| 切换插件标签 | 游戏页「插件」标签内的左侧列表 | 右侧显示所选插件的内容 |  | `lumilio-ui/src/instance_detail/plugin_tabs.rs` |
| 返回插件内容顶部 | 非地图插件内容 · 右下角悬浮「返回顶部」按钮 | 将详情和材料清单共享的滚动区域移回顶部；地图使用填满高度的视口 |  | `lumilio-ui/src/instance_detail/plugin_tabs.rs` |
| 观察 3D 投影 | 3D 预览弹窗画面 | Orbital 拖拽旋转、滚轮缩放；Explore 点击捕获鼠标后第一人称转向，WASD 移动、空格上升、Shift 下降；Esc 先释放鼠标；失焦自动释放 | 自由飞行，不含重力与碰撞；动画方块暂时静止 | `lumilio-ui/src/model_view/mod.rs` |
| 复位 3D 投影视角 | 弹窗「复位视角」 | 当前模式回到初始视角；Orbital 可双击画面，聚焦画面后可按 R |  | `lumilio-ui/src/model_view/mod.rs` |
| 重试 3D 投影预览 | 弹窗出错后的「重试」 | 重新读取当前投影与游戏贴图并生成预览；失败仍显示说明 |  | `lumilio-ui/src/model_view/mod.rs` |
| 切换 3D 投影观察模式 | 预览弹窗 Orbital / Explore 分段 | 两种模式分别保留视角；切换时释放鼠标并停止移动 |  | `lumilio-ui/src/model_view/mod.rs` |
| 退出 3D 投影预览 | 弹窗 Esc / 关闭按钮 | 捕获鼠标时 Esc 先释放并停止移动，再按 Esc 关闭弹窗；关闭后焦点回到入口并释放渲染资源 |  | `lumilio-ui/src/model_view/modal.rs` |
| 打开 3D 投影预览 | 投影详情标题同行的顶部按钮组「打开 3D 预览」 | 打开观察弹窗后才读取投影和游戏贴图；加载、错误与重试均在弹窗内；游戏版本画不出的方块在技术详情中列出 |  | `lumilio-ui/src/model_view/modal.rs` |
