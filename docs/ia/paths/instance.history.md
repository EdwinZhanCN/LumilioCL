<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 游戏页 · 历史 · 已实现的用户路径

表里每一行都有对应的实现；没做的、范围外的见 [../README.md](../README.md)。

| 操作 | 层 / 组件 | 结果与反馈 | 编号 | 备注 | 实现 |
|---|---|---|---|---|---|
| 看变更 | 历史 · 变更分段 | 只读时间线（内容、世界、设置、安装等） | — |  | `lumilio-ui/src/instance_detail/panels/history.rs` |
| 看游玩记录 | 历史 · 游玩记录分段 | 只读；崩溃或没能启动的会话有「查看日志」，跳到诊断·日志，崩溃报告按时间对应（会话开始到结束后 2 分钟内写下、离结束最近的那份），找到就直接打开 | — |  | `lumilio-ui/src/instance_detail/panels/history.rs` |
| 创建快照 | 历史 · 快照分段「现在创建快照」→ 弹窗（备注、范围） | 后台创建 → toast | L-HIST-01 |  | `lumilio-ui/src/instance_detail/panels/history.rs` |
| 恢复快照 | 快照行「恢复」→ 警告弹窗“恢复会用快照替换当前的 X，当前状态会先自动存一份” | 后台恢复 → toast；失败自动回到恢复前 | L-HIST-01 |  | `lumilio-ui/src/instance_detail/panels/history.rs` |
| 删除快照 | 快照行「删除」→ 警告弹窗 | 删除快照文件 | L-HIST-01 |  | `lumilio-ui/src/instance_detail/panels/history.rs` |
