<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 首页 · 已实现的用户路径

表里每一行都有对应的实现；没有的就是没做。

| 操作 | 层 / 组件 | 结果与反馈 | 备注 | 实现 |
|---|---|---|---|---|
| 继续 | 英雄区主按钮「继续」 | 启动当前游戏，英雄区进入启动时刻；跟随当前游戏（导航右下角芯片） |  | `lumilio-ui/src/home/launching.rs` |
| 取消启动 | 启动时刻里的「取消」 | 停止本次启动，回到继续状态 |  | `lumilio-ui/src/home/launching.rs` |
| 结束游戏 | 游戏运行中的「结束游戏」 | 停止游戏进程 |  | `lumilio-ui/src/home/launching.rs` |
| 空库：导入 | 首次使用的「把原来的游戏带过来」 | 同游戏库的导入其他启动器的游戏 |  | `lumilio-ui/src/home/lists.rs` |
| 空库：新建 | 首次使用的「新建」 | 同游戏库的新建游戏 |  | `lumilio-ui/src/home/lists.rs` |
| 需要留意：解决 | 每个有问题的游戏一行（游戏名 · 最严重的问题 · 另有几个）和一个按钮（安装/修复/更换…/去设置/去添加/查看日志…） | 点后先打开游戏页再执行 |  | `lumilio-ui/src/home/lists.rs` |
| 打开最近的游戏 | 最近卡片（手型和悬停描边） | 点卡片进游戏页 |  | `lumilio-ui/src/home/lists.rs` |
| 恢复并继续 | 启动失败后的「恢复并继续」 | 修复后重试启动 |  | `lumilio-ui/src/home/recovery.rs` |
| 技术详情 | 启动失败后的「技术详情」 | 失败原因弹窗 |  | `lumilio-ui/src/home/recovery.rs` |
