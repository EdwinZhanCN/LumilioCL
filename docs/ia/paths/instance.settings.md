<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 游戏页 · 设置 · 已实现的用户路径

表里每一行都有对应的实现；没做的、范围外的见 [../README.md](../README.md)。

| 操作 | 层 / 组件 | 结果与反馈 | 编号 | 备注 | 实现 |
|---|---|---|---|---|---|
| 内存 | 设置 · 性能组，「内存」区 [编辑] 弹窗（最小 / 最大） | 留空跟随默认；性能组另有只读的“本机内存 · 推荐最大”一行 | L-SET-01 |  | `lumilio-ui/src/instance_detail.rs` |
| 窗口大小、全屏 | 设置 · 游戏组，值 + [编辑] 弹窗 | 宽高一起填（留空跟随默认）；全屏 关 / 开 / 跟随默认；恢复默认＝移除覆盖 | L-SET-01 |  | `lumilio-ui/src/instance_settings.rs` |
| 进入游戏后 | 设置 · 游戏组，值 + [编辑] 弹窗 | 保持 / 隐藏启动器 / 跟随默认 | L-SET-01 |  | `lumilio-ui/src/instance_settings.rs` |
| 直接进入（Quick Play） | 设置 · 游戏组，值 + [编辑] 弹窗：类型 + 目标 | 启动后直达世界或服务器；版本不支持单人世界时启动会说明原因；世界页的「进入」用同一个能力 | H-PLAY-07 |  | `lumilio-ui/src/instance_settings.rs` |
| 更换游戏版本 / 加载器 | 设置 · 运行时，「游戏版本」「加载器」行的 [更换…] → 弹窗（加载器分段 + 版本选择器，起点是游戏现在的组合） | 警告“装好的 Mod 可能不兼容，先建快照” + [先建快照]；与现在相同时不能提交；失败回到旧组合 | L-LIB-07 |  | `lumilio-ui/src/instance_settings.rs` |
| 修复游戏文件 | 设置 · 运行时，「游戏文件」行的 [修复] | 逐个核对游戏文件，缺的或损坏的重新下载；世界、Mod 和设置不会被改动；后台任务 | H-INSTANCE-11 |  | `lumilio-ui/src/instance_settings.rs` |
| 指定 Java | 设置 · Java 组，值 + [编辑] 弹窗 | 路径留空自动选择；保存时检查路径存在；已发现的 Java 列表在全局设置·Java | L-RUN-01 |  | `lumilio-ui/src/instance_settings.rs` |
| Java 参数 | 设置 · Java 组，值 + [编辑] 弹窗 | 多行文本，每行一个参数 | L-SET-01 |  | `lumilio-ui/src/instance_settings.rs` |
| 游戏参数 | 设置 · 高级组，值 + [编辑] 弹窗 | 来源（跟随默认 / 自己设置）+ 多行 | L-SET-01 |  | `lumilio-ui/src/instance_settings.rs` |
| 环境变量 | 设置 · 高级组，值 + [编辑] 弹窗 | 来源 + 每行 名称=值 | L-SET-01 |  | `lumilio-ui/src/instance_settings.rs` |
| 启动前 / 包装 / 退出后命令 | 设置 · 高级组，值 + [编辑] 弹窗 | 三个输入：留空跟随默认，填 - 表示这个游戏不使用；说明可用变量 | L-SET-01 |  | `lumilio-ui/src/instance_settings.rs` |
