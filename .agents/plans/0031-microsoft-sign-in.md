# 0031 — Microsoft 登录与账户刷新

- Status: in_progress
- Phase: Phase 8 — ARCH 页面接入和视觉验收
- Author: agent

## Goal

在账户页「登录 Microsoft」：设备码流程登录，账户出现在列表和导航芯片里，启动时用真实的玩家名、UUID 和访问令牌，令牌即将过期时自动刷新，刷新令牌失效时标记「需要重新登录」并明确提示。秘密只存在系统凭据库里（ADR 0013）。

## Scope

- In scope：ADR 0013；`Transport` 增加带头与表单的请求；core `microsoft` 协议客户端（设备码、轮询、刷新、Xbox/XSTS/Minecraft 登录、拥有检查、档案）与每种失败的分类；凭据库抽象（系统实现 + 内存测试实现，无明文回退）；账户模型增加种类与按 UUID 的键；服务层登录/刷新/移除/启动取令牌；`LaunchRequest` 改为携带 `AuthSession`；UI 登录弹窗与账户页/芯片的 Microsoft 状态；诊断包与日志不含令牌；文档。
- Out of scope：第三方认证（authlib-injector）、皮肤、「这一次离线玩」的显式选择、家长/年龄限制之外的账户管理。

## Reference mapping

| LumilioCL | 参考 | 提取 |
|---|---|---|
| core microsoft | `docs/workflows/hmcl-reference.md` H-ACC-01/07、H-PLAY-04；FLOW-REF-035/037 | 设备码授权、令牌链、刷新与失败分支（只取行为，用自己的话实现） |
| 凭据存放 | `docs/app-state.md` §1 | 在线账户秘密进系统凭据库，失败请求登录，不降级写明文 |

## Tasks

- [x] T1: `Transport::send`（方法、头、表单/JSON 体）+ HTTP 实现 + 测试。
- [x] T2: core `microsoft`：设备码、轮询（pending/slow_down/declined/expired/取消）、刷新、令牌链、XErr 分类、拥有检查、档案 + 脚本化服务器测试。
- [x] T3: 凭据库抽象 + 系统实现 + 内存实现；账户模型（种类、键）与 settings 迁移兼容 + 测试。
- [x] T4: 服务层：登录、刷新、移除（同时删凭据）、启动取令牌（缓存/刷新/需重新登录）、`LaunchRequest.session` + 测试，含令牌不进诊断包。
- [x] T5: UI 登录弹窗、账户页与芯片的 Microsoft 状态、复制 UUID、刷新、移除确认；app 接线。
- [x] T6: IA、behavior 文档、四项检查、实机检查（凭据库读写；真实登录需维护者）。
- [ ] T7: 维护者实机登录（真实的 Xbox/Minecraft 步骤与 Mojang 审批）与视觉验收：待维护者检查。

## Validation

四项检查；脚本化服务器覆盖成功链路、每个失败分类、轮询节奏与取消、刷新轮换、缓存令牌复用不发请求、刷新失败后标记需要登录；凭据库不可用时登录失败且不写明文；诊断包不含任何令牌。

## Risks / open questions

- 客户端注册未必已被 Minecraft 服务批准：最后一步会被拒，界面要明确告诉人原因而不是泛泛的「登录失败」。
- Linux 没有 Secret Service 时无法登录；界面说明原因。

## Outcome

- 已用真实服务确认：注册的 client id 能换到设备码（`real_device_code_is_offered_for_the_registered_client`，手动运行）；系统钥匙串读写已在本机验证（`the_system_store_round_trips_a_secret`，手动运行）。Xbox/XSTS/Minecraft 服务这几步只在脚本化服务器上测过，需要你登录一次才能证明真实链路与审批状态。
- core：`Transport::send`、`microsoft` 协议客户端（设备码、轮询、刷新、令牌链、每种失败）、`credentials`（系统/内存）、账户键与种类、服务层登录/刷新/启动取令牌/移除、`LaunchRequest.session`。
- UI：登录弹窗（代码、复制、重新打开页面、取消、失败与重试）、账户页「登录 Microsoft」「刷新登录」「需要重新登录」、导航芯片按键选择。
