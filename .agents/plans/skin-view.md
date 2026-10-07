# 皮肤与披风的完整视图

- Status: proposed

## Goal

账户之外有一个独立的皮肤页面。在线 Microsoft 账户可以查看、穿上、换掉自己的皮肤和已拥有的披风，并把一张本地 PNG 上传到 Mojang。离线账户继续使用现在的本地文件、LittleSkin 或 CustomSkinLoader 选择，启动行为仍按 ADR 0024。两种账户共用同一个可旋转的立体预览，能看出经典宽臂和纤细窄臂、第二层（头发、外套）和披风。这是启动器自己的页面，不是插件。

## Scope

- In：独立皮肤页；在线档案的皮肤列表、自定义库、穿戴、卸下、删除、排序、披风选择；64×32 旧贴图规范成 64×64；拖入 PNG；离线选择接到现有 `SkinChoice`；离屏立体预览。
- Out：Modrinth Pride 等活动皮肤、Ears 模组的额外层、常驻行走动画、把 Steve/Alex 的 PNG 放进仓库、`authlib-injector:` 链接拖入、Bedrock 皮肤、皮肤绘制器。

## 调研

Modrinth 的皮肤功能在 `packages/app-lib/src/state/minecraft_skins/` 和 `apps/app-frontend/src/pages/Skins.vue`，后端命令在 `apps/app/src/api/minecraft_skins.rs`。页本身是主导航里的一页，不是账户对话框。它能列出账户皮肤和自定义皮肤、拖入文件、选择 classic/slim、从已拥有披风里挑选、穿戴、卸下、删除、重排，并把贴图规范化。穿戴后大约 11 秒再向档案确认，因为 Mojang 不会立刻反映变更。预览在 `packages/ui/src/utils/webgl/skin-rendering.ts`，用 WebGL 画皮肤和披风，另外还有 Ears 检测。这些是要对齐的能力。Modrinth 的活动分组和 Ears 不对齐。

Mojang 档案 API（<https://minecraft.wiki/w/Mojang_API>）：

- `GET https://api.minecraftservices.com/minecraft/profile` 返回 `skins[]`（`url`、`variant` 为 `CLASSIC` 或 `SLIM`、`state`）和 `capes[]`（`url`、`alias`、`state`）。
- 上传是 `POST /minecraft/profile/skins`，表单字段 `variant=classic|slim` 和 PNG 文件，要 Bearer 令牌。
- `DELETE /minecraft/profile/skins/active` 恢复默认皮肤。披风也有对应的隐藏请求。
- 未拥有 Minecraft 的账户会得到 `NOT_FOUND`。令牌来自现有 Microsoft 会话（ADR 0020），不新增一套登录。

皮肤贴图是 64×64 PNG。经典臂宽 4 像素，纤细臂宽 3 像素。64×32 是旧版，只含身体、没有第二层。披风是另一张贴图，不是每个人都有。

立体预览不能把 Three.js 搬进 GPUI。玩家模型是几块方盒加一层外套和一块披风，沿用投影预览那条离屏渲染路径：UI 线程只提交贴图和相机，worker 出帧。设计语言不允许为了「活着」而挂一个永远走动的时钟；轨道观察是主动输入。行走只作为可关闭的预览动作，并服从减弱动效。

Steve、Alex 等默认图是 Mojang 的作品。ADR 0024 已拒绝把这些图放进仓库。页面在「卸下皮肤」时，从该账户将要启动的客户端 jar 读取 `assets/minecraft/textures/entity/player/` 下的宽臂和窄臂图；没有已安装版本时，用几何体加说明，不画冒充的 Mojang 贴图。

离线路径已经能选择本地皮肤、披风和模型（`skin_dialog.rs`、`SkinChoice`）。这一页替换那个只有路径输入的对话框，保存结果仍然是 `SkinChoice`，启动时的本地 Yggdrasil 与 authlib-injector 不变。

## References

- `3rd-party/modrinth/packages/app-lib/src/state/minecraft_skins/mod.rs`
- `3rd-party/modrinth/packages/app-lib/src/state/minecraft_skins/mojang_api.rs`
- `3rd-party/modrinth/apps/app-frontend/src/pages/Skins.vue`
- `3rd-party/modrinth/packages/ui/src/utils/webgl/skin-rendering.ts`
- ADR 0024、ADR 0020
- Mojang API：<https://minecraft.wiki/w/Mojang_API>

## Tasks

- [ ] T1：core 增加 Microsoft 档案的读取、上传、穿戴、卸下和披风切换。令牌只从凭据库来。失败分成未拥有游戏、令牌失效、图片不合格、网络失败。上传后的确认允许档案延迟，而不是立刻把一次旧档案当成失败。
- [ ] T2：贴图规范化与模型判断：64×64、64×32 升级、拒绝其余尺寸。自定义库记录顺序、模型和披风 id，存在启动器数据目录，不写进实例。
- [ ] T3：独立皮肤页。在线账户看到正在使用的皮肤、账户皮肤、自定义库和披风。离线账户看到默认、本地文件、LittleSkin、CustomSkinLoader，字段与现在的对话框一致。拖入 PNG 进入自定义库。
- [ ] T4：离屏预览。宽臂与窄臂、第二层、披风、拖拽旋转。减弱动效时没有行走循环。关闭页面释放贴图和 worker。
- [ ] T5：默认外观从已安装游戏 jar 读取；读不到就显示几何体和一句说明。

## Validation

- 脚本化档案服务器覆盖列表、上传、穿戴、卸下、披风切换，以及未拥有游戏和过期令牌。
- 64×32 输入得到 64×64 输出；错误尺寸被拒绝且不上传。
- 离线三种 `SkinChoice` 仍能启动 ADR 0024 的本地皮肤服务器。
- UI 测试覆盖切账户、拖入、排序、关闭后重开不会把上一张贴图画进新页面。
- 实机：一个真实 Microsoft 账户换肤并在游戏里看见；一个离线账户选本地皮肤和披风，预览与游戏一致。

## Open questions

- 第三方 Yggdrasil 服务器的上传接口不统一。这一页只保证 LittleSkin / CSL 的现有选择，不承诺向任意皮肤站上传。
- 披风的鞘翅形态只在预览里可选，不写入游戏设置。
