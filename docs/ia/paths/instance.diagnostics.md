<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 游戏页 · 诊断 · 已实现的用户路径

表里每一行都有对应的实现；没有的就是没做。

| 操作 | 层 / 组件 | 结果与反馈 | 备注 | 实现 |
|---|---|---|---|---|
| 浏览文件 | 诊断 · 文件分段 | 逐级打开的只读文件夹列表，只在游戏目录内；右上角按名字搜当前文件夹 |  | `lumilio-ui/src/instance_detail/diagnostics/files.rs` |
| 在访达中显示 | 文件分段 · 按键 | 当前文件夹或所选文件 |  | `lumilio-ui/src/instance_detail/diagnostics/files.rs` |
| 实时日志（游戏运行中） | 诊断 · 日志分段 | 本启动器启动的游戏运行时显示它正在输出的内容（最近 2000 行），自动跟随末尾；游戏结束后改读 latest.log；启动器之外启动的游戏没有实时输出 |  | `lumilio-ui/src/instance_detail/diagnostics/logs.rs` |
| 按级别筛选 | 日志分段 · 分段 | 视图状态；级别＝该级别及更严重，堆栈行跟随上一行的级别 |  | `lumilio-ui/src/instance_detail/diagnostics/logs.rs` |
| 搜索日志 | 日志分段 · 搜索框 | 视图状态 |  | `lumilio-ui/src/instance_detail/diagnostics/logs.rs` |
| 复制日志 | 日志分段 · 按键「复制」 | 复制当前筛选出的行；没有行时提示 |  | `lumilio-ui/src/instance_detail/diagnostics/logs.rs` |
| 导出日志 | 日志分段 · 按键「导出…」→ 选位置 | 保存最新日志或打开着的崩溃报告；玩家名、UUID、启动器目录、游戏目录、用户主目录都换成占位符 |  | `lumilio-ui/src/instance_detail/diagnostics/logs.rs` |
| 崩溃原因识别 | 崩溃报告上方的原因卡片 | 显示已启用分析器给出的原因、建议与可展开的日志证据；停用后不再显示 |  | `lumilio-ui/src/instance_detail/diagnostics/logs.rs` |
| 看最新日志 / 崩溃报告 | 日志分段 · 来源选择：最近的日志 / 崩溃报告列表 | 选哪份看哪份；崩溃报告「查看」打开 |  | `lumilio-ui/src/instance_detail/diagnostics/logs.rs` |
| 看问题、执行修复 | 诊断 · 问题分段 | 每个问题一个操作（与概览相同） |  | `lumilio-ui/src/instance_detail/diagnostics/mod.rs` |
