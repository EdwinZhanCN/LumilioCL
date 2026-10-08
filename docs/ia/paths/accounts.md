<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 账户 · 已实现的用户路径

表里每一行都有对应的实现；没有的就是没做。

| 操作 | 层 / 组件 | 结果与反馈 | 备注 | 实现 |
|---|---|---|---|---|
| 查看账户 | 左侧列表的一行 | 右侧显示该账户的详情与外观；不改变当前账户 |  | `lumilio-ui/src/pages/live/accounts.rs` |
| 复制 UUID | 详情 ⋯ 菜单 | 复制到剪贴板，toast“已复制 UUID” |  | `lumilio-ui/src/pages/live/accounts.rs` |
| 刷新登录 | 已登录账户详情 ⋯ 菜单「刷新登录」 | 失效的登录显示“需要重新登录”，刷新后恢复 |  | `lumilio-ui/src/pages/live/accounts.rs` |
| 移除 | 详情 ⋯ 菜单 → 警告弹窗 | 只删身份（已登录账户同时删凭据库里的登录信息），不删游戏和存档；移除当前账户后改用剩下的第一个 |  | `lumilio-ui/src/pages/live/accounts.rs` |
| 设为当前 | 详情页头「设为当前账户」 | 之后的启动使用该身份；导航芯片也跟着换 |  | `lumilio-ui/src/pages/live/accounts.rs` |
| 设置皮肤 | 离线账户详情「外观」里的「更改…」→ 弹窗 | 选默认、本地文件、LittleSkin 或自定义皮肤站；预览随之更新；游戏里按所选显示（启动时在本机起一个皮肤服务器，需要 authlib-injector）；加载不到时游戏照常启动并在日志里说明 | ADR 0024 | `lumilio-ui/src/pages/live/accounts.rs` |
| 添加离线账户 | L2 次要「添加离线账户」→ 弹窗（可自定义 UUID） | 列表新增；UUID 只在添加时可设 |  | `lumilio-ui/src/pages/live/accounts.rs` |
| Microsoft 登录 | L2 主要「登录 Microsoft」→ 设备码弹窗 | 登录后出现在列表和导航芯片里，启动用真实的玩家名和令牌 | ADR 0020 | `lumilio-ui/src/pages/live/accounts.rs` |
| 第三方登录 | 页头 ⋯ 菜单「第三方登录…」→ 弹窗 | 选认证服务器（内置 LittleSkin）、输入账号密码；多个角色时选一个；登录后出现在列表里，启动时自动加载 authlib-injector | ADR 0024 | `lumilio-ui/src/pages/live/accounts.rs` |
| 管理认证服务器 | 页头 ⋯ 菜单「认证服务器…」→ 弹窗 | 添加（输入地址，先看到名称，http 有警告）、移除（会一并移除该服务器上的账户）；LittleSkin 内置 |  | `lumilio-ui/src/pages/live/accounts.rs` |
| 观察外观 | 详情里的立体预览 | 拖动或方向键旋转，滚轮或 + / − 缩放，双击或 R 复位；第二层与披风一起画 |  | `lumilio-ui/src/skin_view/mod.rs` |
