<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 账户 · 已实现的用户路径

表里每一行都有对应的实现；没做的、范围外的见 [../README.md](../README.md)。

| 操作 | 层 / 组件 | 结果与反馈 | 编号 | 备注 | 实现 |
|---|---|---|---|---|---|
| 选为当前 | 账户行左侧的单选圆点 | 之后的启动使用该身份 | H-ACC-06 |  | `lumilio-ui/src/pages/live/accounts.rs` |
| 复制 UUID | 账户行 ⋯ 菜单 | 复制到剪贴板，toast“已复制 UUID” | H-ACC-09 |  | `lumilio-ui/src/pages/live/accounts.rs` |
| 移除 | 账户行 ⋯ 菜单 → 警告弹窗 | 只删身份（Microsoft 账户同时删凭据库里的登录信息），不删游戏和存档；移除当前账户后改用剩下的第一个 | H-ACC-08 |  | `lumilio-ui/src/pages/live/accounts.rs` |
| 刷新登录 | Microsoft 账户行 ⋯ 菜单「刷新登录」 | 失效的登录显示“需要重新登录”，刷新后恢复 | H-ACC-07 |  | `lumilio-ui/src/pages/live/accounts.rs` |
| 添加离线账户 | L2 次要「添加离线账户」→ 弹窗（可自定义 UUID） | 列表新增；UUID 只在添加时可设 | H-ACC-02 |  | `lumilio-ui/src/pages/live/accounts.rs` |
| Microsoft 登录 | L2 主要「登录 Microsoft」→ 设备码弹窗 | 登录后出现在列表和导航芯片里，启动用真实的玩家名和令牌 | H-ACC-01 | ADR 0013；真实登录要 Mojang 批准应用注册 | `lumilio-ui/src/pages/live/accounts.rs` |
