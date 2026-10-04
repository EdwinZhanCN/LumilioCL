# 第三方认证、离线皮肤与内置 LittleSkin

- Status: in_progress

## Goal

账户页可以添加 authlib-injector 认证服务器（内置 LittleSkin），用 Yggdrasil 登录，启动时注入 authlib-injector；离线账户可以选皮肤（本地文件 / LittleSkin / 自定义 CSL 皮肤站），由启动器在本机起一个最小皮肤服务器。业务逻辑以 HMCL 为准（用户的明确要求）。

## Scope

- In：Yggdrasil 客户端（authenticate / refresh / validate / invalidate）；服务器发现（`x-authlib-injector-api-location`）与元数据；内置 LittleSkin；多角色选择；authlib-injector 首次使用时下载并校验；启动参数注入（含 prefetched 元数据）；离线皮肤（local file、LittleSkin CSL、自定义 CSL）与本机 Yggdrasil 服务器（含 SHA1withRSA 签名）；UI；ADR 0018 的收尾（替换为新 ADR）。
- Out：上传皮肤到认证服务器（HMCL 有，另议）；Steve/Alex 等内置默认皮肤（要带 Mojang 贴图，或从游戏 jar 里取，另议）；`authlib-injector:` 协议拖拽添加；除 LittleSkin 外的预置服务器。

## References

- `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/auth/yggdrasil/`（`YggdrasilService`、`YggdrasilAccount`、`YggdrasilSession`）
- `.../auth/authlibinjector/`（`AuthlibInjectorServer`、`AuthlibInjectorAccount`、`AuthlibInjectorProvider`、`AuthlibInjectorDownloader`）
- `.../auth/offline/`（`OfflineAccount`、`Skin`、`Texture`、`YggdrasilServer`）
- `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/setting/AuthlibInjectorServerList.java`（LittleSkin：`https://littleskin.cn/api/yggdrasil/`）、`ui/account/OfflineAccountSkinPane.java`、`AddAuthlibInjectorServerPane.java`、`setting/Accounts.java`（错误话术）
- ADR 0018、0020、0011（HMCL 派生，AGPL）

## HMCL 的规则（实现照此）

- 服务器地址：补 `https://`；GET 一次，若响应头 `x-authlib-injector-api-location` 指向别处就改用它；末尾补 `/`；响应是元数据 JSON：`meta.serverName`、`meta.links`、`meta.feature.non_email_login`（登录名标签「用户名」而不是「邮箱」）。`http://` 地址要警告。
- 接口：`<root>authserver/{authenticate,refresh,validate,invalidate}`、`<root>sessionserver/session/minecraft/profile/<无连字符 uuid>`。认证请求 `agent:{name:Minecraft,version:1}`、`requestUser:true`、随机 `clientToken`，响应里的 `clientToken` 必须与发出的一致。
- 登录后没有选中角色：有可选角色就让人选（没有则「没有角色」）；选定后用 `refresh` 带 `selectedProfile`，响应的选中角色必须就是所选。
- 启动取令牌：先 `validate`；无效再 `refresh`（不带角色）；`refresh` 被 `ForbiddenOperationException` 拒绝 = 登录过期，要重新登录；换了角色或没有角色名都当响应有问题。重新用密码登录时，角色不在了 = 「角色已被删除」。
- 启动参数：`-javaagent:<jar>=<api root>`、`-Dauthlibinjector.side=client`、`-Dauthlibinjector.yggdrasil.prefetched=<元数据的 base64>`；游戏的 `user_type` 用 `msa`，`user_properties` 用登录响应里的 user.properties（`{名: [值]}`）。
- 元数据与 authlib-injector 与登录并行取；取不到不能静默当离线（沿用 ADR 0020）。
- authlib-injector：`https://authlib-injector.yushi.moe/artifact/latest.json`（`build_number`、`version`、`download_url`、`checksums.sha256`），本地版本不低于最新则不下；有镜像规则时走镜像（BMCLAPI 把该域名换成 `<api>/mirrors/authlib-injector`）。HMCL 把 jar 打进安装包，我们首次使用时下载并校验 sha256，不进仓库。
- 离线皮肤类型：默认、本地文件（皮肤 / 披风 / 模型 wide|slim）、LittleSkin（`https://littleskin.cn/csl/<玩家名>.json`，贴图在 `<csl>/textures/<hash>`）、自定义 CSL（同格式，地址补 https、去末尾 `/`）。JSON：`username` 非空才算有皮肤；`textures.slim` 有 → slim 模型取该哈希，`textures.default` 有 → wide 取该哈希，否则回退 `skin`；披风 `textures.cape` 或 `cape`。
- 本机服务器（离线且皮肤不是默认时才起，随游戏结束停）：`GET /`（元数据：`signaturePublickey` PEM、`skinDomains: [127.0.0.1, localhost]`、`meta.serverName`、`feature.non_email_login`）、`GET /status`、`POST /api/profiles/minecraft`、`GET /sessionserver/session/minecraft/hasJoined?username=`、`POST /sessionserver/session/minecraft/join`（204）、`GET /sessionserver/session/minecraft/profile/<uuid>`、`GET /textures/<sha256 hex>`（`image/png`、`Etag`、`Cache-Control: max-age=2592000, public`）。角色响应里 `properties:[{name:textures,value:<base64 JSON>,signature}]`，JSON 含 `timestamp`、`profileId`、`profileName`、`textures:{SKIN:{url,metadata:{model:slim}},CAPE:{url}}`；签名 SHA1withRSA。
- 失败话术（HMCL `Accounts.accountException`）：连不上 / SSL / 响应格式不对 / 无效凭据 / 令牌无效 / 迁移（资源已不可用）/ authlib-injector 下载失败 / 角色已删除。

## Tasks

- [x] T1: core `yggdrasil`：客户端 + 错误分类 + 脚本化服务器测试。
- [x] T2: 服务器发现、元数据、内置 LittleSkin；设置里的自定义服务器。
- [x] T3: authlib-injector 工件：最新版查询、下载、sha256 校验、缓存、镜像。
- [x] T4: 账户模型（第三方种类、服务器、登录名）与凭据（clientToken / accessToken / userProperties）；服务层登录、选角色、刷新、移除、启动取会话；`AuthSession` 带注入。
- [x] T5: 启动注入（JVM 参数）。
- [ ] T6: 离线皮肤：设置里的皮肤选择、加载（本地 / CSL）、纹理哈希、本机 Yggdrasil 服务器与签名、随游戏生灭。
- [ ] T7: UI：服务器管理、第三方登录弹窗（含选角色）、账户行与菜单、离线账户「皮肤」弹窗；app 接线。
- [ ] T8: ADR（取代 0018）、IA、四项检查、更新 backlog 的「只有维护者能做」。

## Validation

四项检查；脚本化服务器覆盖：登录成功、选角色、各失败分类、令牌校验/刷新/过期、服务器发现重定向；本机皮肤服务器用真实 TCP 请求测每个路由和签名可验证；注入参数；令牌不进日志/诊断包/设置。需要维护者：用真实 LittleSkin 账号登录并进游戏看皮肤。

## Open questions

- 本机服务器的 RSA 密钥：每次启动现生成（HMCL 做法）还是存进数据目录；倾向存到数据目录一次生成，免得每次启动等密钥生成。
