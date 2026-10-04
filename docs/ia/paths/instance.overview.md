<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 游戏页 · 概览 · 已实现的用户路径

表里每一行都有对应的实现；没做的、范围外的见 [../README.md](../README.md)。

| 操作 | 层 / 组件 | 结果与反馈 | 编号 | 备注 | 实现 |
|---|---|---|---|---|---|
| 改名 | 概览「名称」行 [编辑] → 弹窗 | 改名保留游戏目录、收藏和历史记录 | L-LIB-04 |  | `lumilio-ui/src/instance_detail/overview.rs` |
| 占用空间 | 概览「占用空间」行 | 后台计算，只算这个游戏自己的文件 | — |  | `lumilio-ui/src/instance_detail/overview.rs` |
| 最近游玩 / 变更 | 概览里的两个只读摘要，各 3 条 | “查看全部”进历史分段 | L-HIST-01 |  | `lumilio-ui/src/instance_detail/panels.rs` |
| 解决问题 | 概览「需要留意」里每个问题行的按钮 | 安装/修复/更换版本，或跳到设置·Java、设置·性能、内容、账户、诊断·日志 | L-DIAG-01 |  | `lumilio-ui/src/instance_detail/panels.rs` |
