<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 游戏页 · 诊断 · 已实现的用户路径

表里每一行都有对应的实现；没有的就是没做。

| 操作 | 层 / 组件 | 结果与反馈 | 备注 | 实现 |
|---|---|---|---|---|
| 复制分析技术详情 | 崩溃分析弹窗 · 按键「复制技术详情」 | 复制来源、分析结果、证据与点击时的完整日志快照，内容已脱敏；加载和失败时禁用 |  | `lumilio-ui/src/instance_detail/diagnostics/analysis.rs` |
| 浏览文件 | 诊断 · 文件分段 | 逐级打开的只读文件夹列表，只在游戏目录内；右上角按名字搜当前文件夹 |  | `lumilio-ui/src/instance_detail/diagnostics/files.rs` |
| 在访达中显示 | 文件分段 · 按键 | 当前文件夹或所选文件 |  | `lumilio-ui/src/instance_detail/diagnostics/files.rs` |
| 选择日志来源 | 日志工具栏 · 搜索框右侧单选下拉 | 实时输出、latest.log、历史日志（含 .log.gz）、崩溃报告共用一个阅读区；可搜索来源；选择文件后显示加载或读取失败 |  | `lumilio-ui/src/instance_detail/diagnostics/logs.rs` |
| 按级别筛选 | 日志工具栏 · 来源右侧多选下拉 | 默认全部，点击或 Enter 独立切换错误、警告、信息、调试；菜单保持打开，Escape 关闭并保留选择；堆栈继承上一行级别；崩溃报告禁用级别筛选 |  | `lumilio-ui/src/instance_detail/diagnostics/logs.rs` |
| 崩溃分析 | 日志工具栏 · 按键「崩溃分析…」→ 弹窗 | 分析当前完整来源的快照；显示可能原因、建议、可展开证据；无匹配或失败明确提示 |  | `lumilio-ui/src/instance_detail/diagnostics/logs.rs` |
| 复制日志 | 日志工具栏 · 按键「复制」 | 复制当前来源筛选出的全部行；无匹配时提示 |  | `lumilio-ui/src/instance_detail/diagnostics/logs.rs` |
| 导出日志 / 报告 | 日志工具栏 · 按键「导出…」→ 选位置 | 导出当前完整来源，忽略阅读筛选；压缩日志解压为文本；玩家名、UUID 和目录脱敏；实时来源保存点击时的输出快照 |  | `lumilio-ui/src/instance_detail/diagnostics/logs.rs` |
| 搜索日志 | 日志工具栏 · 搜索框 | 在当前来源中忽略大小写筛选 |  | `lumilio-ui/src/instance_detail/diagnostics/logs.rs` |
| 回到底部 | 日志状态行 · 按键 | 回到实时输出末尾并恢复跟随；向上滚动暂停跟随 |  | `lumilio-ui/src/instance_detail/diagnostics/logs.rs` |
| 看问题、执行修复 | 诊断 · 问题分段 | 每个问题一个操作（与概览相同） |  | `lumilio-ui/src/instance_detail/diagnostics/mod.rs` |
