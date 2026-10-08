# 皮肤与披风：账户详情里的外观

- Status: in_progress

## Goal

账户页改成列表加详情。选中一个账户，右侧显示它的外观：一个能拖动旋转、缩放的立体预览，旁边是按账户种类变化的衣橱。正版 Microsoft 账户可以上传本地 PNG 到 Mojang、从本地皮肤库里换、恢复默认，并在已拥有的披风里选一件或不穿。离线账户在这里选默认、本地文件、LittleSkin 或自定义皮肤站，启动行为仍按 ADR 0024。第三方账户只预览皮肤站上的样子，并能打开皮肤站去改。这是启动器自己的页面，不是插件。

## Scope

- In：账户页的列表加详情；CPU 光栅化的立体预览（宽臂、窄臂、第二层、披风，鞘翅形态只在预览里）；Microsoft 档案的读取、上传、恢复默认与披风切换；本地皮肤库（顺序、模型、来源）；64×32 旧贴图升级成 64×64；拖入 PNG；离线选择接到现有 `SkinChoice`；第三方账户的只读预览；默认外观从已安装游戏的 jar 读取。
- Out：独立的导航地标、Modrinth 的活动皮肤与 Ears 额外层、常驻行走动画、把 Steve / Alex 的 PNG 放进仓库、`authlib-injector:` 链接拖入、Bedrock 皮肤、皮肤绘制器、向任意第三方皮肤站上传、OptiFine 披风。

## 已定

- **信息架构（维护者 2026-10-08）。** 不加地标。账户页像设置的插件页签那样分成左侧列表与右侧详情；页头和页面动作不变。详情的头部是名字、种类和 UUID，下面是预览与衣橱。账户行 ⋯ 菜单里的「皮肤…」去掉；导航的账户下拉可以直接打开当前账户的详情。
- **渲染（维护者 2026-10-08）。** CPU 光栅化，不用 GPU。新 crate `lumilio-skin-render` 不认识 GPUI 和启动器：输入 64×64 RGBA 贴图、手臂模型、可选披风、姿态、相机和画面尺寸，输出 BGRA 像素。最近邻采样，第二层按游戏的做法略大一圈，并用 alpha 剔除。UI 沿用投影预览的出帧方式：worker 线程出帧，新请求覆盖旧请求，只在相机、尺寸或贴图变化时重画，替换掉的帧从 GPUI 的图片缓存里移除（ADR 0028）。交互与投影预览的轨道模式一致：拖动旋转、滚轮缩放、方向键与 + / −、双击复位。

## 调研

Modrinth 的皮肤功能在 `packages/app-lib/src/state/minecraft_skins/` 和 `apps/app-frontend/src/pages/Skins.vue`，后端命令在 `apps/app/src/api/minecraft_skins.rs`。它能列出账户皮肤和自定义皮肤、拖入文件、选择 classic / slim、从已拥有披风里挑选、穿戴、卸下、删除、重排，并把贴图规范化。穿戴后大约 11 秒再向档案确认，因为 Mojang 不会立刻反映变更。预览在 `packages/ui/src/utils/webgl/skin-rendering.ts`，用 WebGL。HMCL 用 JavaFX 3D 画（`HMCL/src/main/java/org/jackhuang/hmcl/ui/account/` 下的皮肤画布）。

Mojang 档案 API（<https://minecraft.wiki/w/Mojang_API>）：

- `GET https://api.minecraftservices.com/minecraft/profile` 返回 `skins[]`（`url`、`variant` 为 `CLASSIC` 或 `SLIM`、`state`）和 `capes[]`（`id`、`url`、`alias`、`state`）。
- 上传是 `POST /minecraft/profile/skins`，表单字段 `variant=classic|slim` 和 PNG 文件，要 Bearer 令牌。`DELETE /minecraft/profile/skins/active` 恢复默认皮肤。
- 披风只能在已拥有的里选：`PUT /minecraft/profile/capes/active`（`capeId`），`DELETE /minecraft/profile/capes/active` 不穿。不能上传自定义披风。
- 未拥有 Minecraft 的账户会得到 `NOT_FOUND`。令牌来自现有 Microsoft 会话（ADR 0020）。

游戏按 UUID 向会话服务器取档案，档案里签过名的 `textures` 给出皮肤与披风地址和手臂模型；正版服务器把它转给别的玩家。离线账户没有档案，1.19.3 起按 UUID 在 9 个默认角色（各有宽臂、窄臂）里挑一个；自定义皮肤靠启动时的本机 Yggdrasil 与 authlib-injector（ADR 0024），只有自己看得见。

皮肤贴图是 64×64 PNG。经典臂宽 4 像素，纤细臂宽 3 像素。64×32 是 1.8 以前的旧版，只有第一层，手臂与腿左右共用。披风贴图 64×32，鞘翅用同一张。

Steve、Alex 等默认图是 Mojang 的作品，ADR 0024 已拒绝把它们放进仓库。没有选皮肤时从该账户将要启动的客户端 jar 读取 `assets/minecraft/textures/entity/player/` 下的图；没有已安装版本时画没有贴图的灰色模型并加一句说明。

## References

- `3rd-party/modrinth/packages/app-lib/src/state/minecraft_skins/mod.rs`
- `3rd-party/modrinth/packages/app-lib/src/state/minecraft_skins/mojang_api.rs`
- `3rd-party/modrinth/apps/app-frontend/src/pages/Skins.vue`
- `3rd-party/modrinth/packages/ui/src/utils/webgl/skin-rendering.ts`
- ADR 0020、ADR 0024、ADR 0028、ADR 0036；设计语言「Native model preview」
- Mojang API：<https://minecraft.wiki/w/Mojang_API>

## Tasks

- [x] T1：`lumilio-skin-render`：玩家模型的方盒与贴图坐标（宽臂、窄臂、第二层、披风），带深度缓冲与 alpha 剔除的软件光栅化，轨道相机，BGRA 输出，灯光跟着相机。像素测试用程序生成的「每面一色」皮肤：脸的方向（左上角标记）、背面与侧面、透明第二层露出第一层、窄臂更窄、背面看到披风外侧、无贴图时是灰色模型。`LUMILIO_SKIN_PREVIEW=<目录>` 时把几个角度写成 PNG 供人看，已看过：比例、朝向、披风位置都对。
- [ ] T1b：鞘翅形态（只在预览里切换）。
- [x] T2：core 的贴图规范化：64×64 原样、64×32 升级（复制手臂与腿到左侧）、其余尺寸拒绝；判断是否像窄臂贴图。
- [ ] T3：账户页改成列表加详情；预览控件（worker 出帧、拖动、缩放、复位、可访问性标签）；离线账户的选择从 `skin_dialog` 搬进详情，保存仍是 `SkinChoice`。
- [ ] T4：core 的 Microsoft 档案：读取、上传、恢复默认、披风切换与不穿；失败分成未拥有游戏、令牌失效、图片不合格、网络失败；上传后的确认允许档案延迟。本地皮肤库（顺序、模型、来源）存在启动器数据目录。
- [ ] T5：Microsoft 账户的衣橱：当前皮肤、本地皮肤库、拖入 PNG、上传、恢复默认、已拥有披风。
- [ ] T6：第三方账户的只读预览与「在皮肤站修改」；默认外观从已安装游戏 jar 读取，读不到时灰色模型加说明。
- [ ] T7：导航账户下拉直达当前账户详情；IA 路径与设计语言更新。

## Validation

- 渲染的像素测试（T1）；64×32 输入得到 64×64 输出，错误尺寸被拒绝且不上传。
- 脚本化档案服务器覆盖读取、上传、恢复默认、披风切换，以及未拥有游戏和过期令牌。
- 离线三种 `SkinChoice` 仍能启动 ADR 0024 的本地皮肤服务器。
- UI 测试覆盖切账户、拖入、关闭后重开不会把上一张贴图画进新详情。
- 实机：一个真实 Microsoft 账户换肤并在游戏里看见；一个离线账户选本地皮肤和披风，预览与游戏一致。

## Open questions

- 第三方 Yggdrasil 服务器的上传接口不统一。这一轮只保证 LittleSkin / CSL 的现有选择，不承诺向任意皮肤站上传。
