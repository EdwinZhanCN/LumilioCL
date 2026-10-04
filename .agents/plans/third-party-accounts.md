# 第三方认证、离线皮肤与内置 LittleSkin

- Status: proposed

## Goal

账户页可添加 authlib-injector 认证服务器（内置 LittleSkin），用 Yggdrasil 登录，启动时注入 authlib-injector；离线账户可选皮肤（本地皮肤服务器）。业务逻辑以 HMCL 为准。

## Scope

- In：服务器发现与元数据；Yggdrasil 认证、刷新、选角色；令牌入系统凭据库（ADR 0020）；authlib-injector 首次使用时下载并校验；LittleSkin 预置；离线皮肤（本地服务器 + 注入）；UI；ADR。
- Out：其他预置服务器；Microsoft 登录改动。

## References

- `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/auth/authlibinjector/`、`auth/yggdrasil/`、`auth/offline/`
- `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/setting/AuthlibInjectorServers.java`、`AuthlibInjectorServerList.java`

## Tasks

- [ ] T1: 读 HMCL，写出业务规则对照（本计划追加）。

## Open questions

- 见 T1 之后补充。
