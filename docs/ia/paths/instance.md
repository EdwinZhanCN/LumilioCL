<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 游戏页（整体） · 已实现的用户路径

表里每一行都有对应的实现；没做的路径写在同目录手写的页面文档里。

| 操作 | 层 / 组件 | 结果与反馈 | 编号 | 备注 | 实现 |
|---|---|---|---|---|---|
| 结束游戏 | 本启动器启动的游戏运行时，页头主按钮变「结束游戏」 | 终止进程，会话写入历史 | H-PLAY-09 |  | `lumilio-ui/src/instance_detail.rs` |
| 开始游戏 | 页头主按钮「启动游戏」 | 首页启动时刻接管 | L-PLAY-01 |  | `lumilio-ui/src/instance_detail.rs` |
| 安装游戏文件 | 页头 ⋯ 菜单（游戏还没装好时才有） | 后台任务，动态可见；完成 toast | L-OPS-02 |  | `lumilio-ui/src/instance_detail.rs` |
| 设为当前游戏 | 页头 ⋯ 菜单 | 导航右段芯片跟着换；之后的启动指向它 | H-NAV-03 |  | `lumilio-ui/src/instance_detail.rs` |
| 修复游戏文件 | 页头 ⋯ 菜单 | 核对并补齐/重下损坏文件，后台任务，动态可见 | H-INSTANCE-11 | 运行中或没装好时禁用 | `lumilio-ui/src/instance_detail.rs` |
| 创建快照 | 页头 ⋯ 菜单 → 弹窗（备注可空；范围＝全部或某个世界） | 后台创建，历史·快照可见 | L-HIST-01 |  | `lumilio-ui/src/instance_detail.rs` |
| 在访达中显示 | 页头 ⋯ 菜单 | 打开游戏目录 | H-INSTANCE-10 |  | `lumilio-ui/src/instance_detail.rs` |
| 复制游戏 | 页头 ⋯ 菜单 → 弹窗（新名称、是否复制存档） | 后台复制成独立副本；不复制历史、快照和游玩时间 | L-LIB-05 |  | `lumilio-ui/src/instance_detail.rs` |
| 完整备份 | 页头 ⋯ 菜单 → 选位置 | 后台打成一个 zip（含存档，不含日志），toast；之后可在游戏库「从备份恢复」 | — | ADR 0015；运行中禁用 | `lumilio-ui/src/instance_detail.rs` |
| 导出整合包 | 页头 ⋯ 菜单 → 导出弹窗（格式、勾选文件） | 后台导出，可取消 | L-LIB-08 |  | `lumilio-ui/src/instance_detail.rs` |
| 删除游戏 | 页头 ⋯ 菜单 → 警告弹窗 | 删除后从历史中移除并后退 | L-LIB-06 |  | `lumilio-ui/src/instance_detail.rs` |
