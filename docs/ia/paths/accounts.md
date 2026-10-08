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
| 在皮肤站修改 | 第三方账户只读衣橱 | 打开服务器公布的主页（LittleSkin 打开其网站）；皮肤与披风只从会话档案预览 |  | `lumilio-ui/src/pages/live/accounts.rs` |
| 添加离线账户 | L2 次要「添加离线账户」→ 弹窗（可自定义 UUID） | 列表新增；UUID 只在添加时可设 |  | `lumilio-ui/src/pages/live/accounts.rs` |
| Microsoft 登录 | L2 主要「登录 Microsoft」→ 设备码弹窗 | 登录后出现在列表和导航芯片里，启动用真实的玩家名和令牌 | ADR 0020 | `lumilio-ui/src/pages/live/accounts.rs` |
| 第三方登录 | 页头 ⋯ 菜单「第三方登录…」→ 弹窗 | 选认证服务器（内置 LittleSkin）、输入账号密码；多个角色时选一个；登录后出现在列表里，启动时自动加载 authlib-injector | ADR 0024 | `lumilio-ui/src/pages/live/accounts.rs` |
| 管理认证服务器 | 页头 ⋯ 菜单「认证服务器…」→ 弹窗 | 添加（输入地址，先看到名称，http 有警告）、移除（会一并移除该服务器上的账户）；LittleSkin 内置 |  | `lumilio-ui/src/pages/live/accounts.rs` |
| 保存离线外观 | 衣橱内联表单 | 默认、本地皮肤与披风、LittleSkin 或自定义站保存为 SkinChoice；预览更新，启动仍按 ADR 0024 |  | `lumilio-ui/src/skin_dialog.rs` |
| 观察外观 | 详情里的立体预览 | 拖动或方向键旋转，滚轮或 + / − 缩放，双击或 R 复位；第二层与披风一起画 |  | `lumilio-ui/src/skin_view/mod.rs` |
| 披风 / 鞘翅预览 | 立体预览下的分段按键（有披风时） | 同一贴图在披风与鞘翅形态间切换，保留相机；只改变预览，不改变账户穿戴 |  | `lumilio-ui/src/skin_view/mod.rs` |
| 没有可用默认贴图 | 未安装游戏或客户端 jar 无默认皮肤 | 灰色模型与说明；安装游戏后重新打开详情读取贴图 |  | `lumilio-ui/src/skin_view/mod.rs` |
| 导入皮肤 | 衣橱「导入 PNG…」或拖入 PNG | 验证并复制到本地库，旧版贴图升级；选择手臂模型后穿戴 |  | `lumilio-ui/src/wardrobe.rs` |
| 更换正版外观 | 衣橱当前皮肤、恢复默认、披风列表 | 用该账户的 Mojang 档案上传与切换；接受后延迟确认，旧档案显示同步中 |  | `lumilio-ui/src/wardrobe.rs` |
| 管理本地库 | 每张皮肤的模型、穿戴与 ⋯ 菜单 | 模型、来源和顺序持久保存；移除库条目不破坏离线账户已选的文件 |  | `lumilio-ui/src/wardrobe.rs` |
