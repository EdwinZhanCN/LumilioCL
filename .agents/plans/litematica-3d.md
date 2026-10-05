# Litematica 投影的 3D 预览

- Status: in_progress

## Goal

投影详情里能看到真实贴图的 3D 预览，可以旋转、缩放。预览由宿主用 webview 里的 schematic-renderer 画出来；插件只在视图树里说「这里有一个 3D 模型」，不接触 webview。贴图来自用户自己那份游戏的 `client.jar`，启动器不分发 Mojang 资源。

## Scope

- In:
  - 先做可行性验证（V1–V5），结果写进实施记录；全部通过才进入实现。
  - 宿主能力：webview 承载 schematic-renderer，从 `client.jar` 生成资源包，`Model` 视图组件。
  - Litematica 详情接入预览。
- Out:
  - 皮肤 3D 预览（backlog 里另有一行，共用「从游戏 jar 取资源」这一层即可，但不在本计划）。
  - 编辑投影、图层动画、实体渲染。
  - 其他格式（`.schem`、`.nbt`）：库支持，但本计划只接 `.litematic`。
  - WASM 社区插件。

## 已定的方向

- 方案：[schematic-renderer](https://github.com/Schem-at/schematic-renderer)（Three.js + Rust/WASM，AGPL-3.0，单文件 UMD，吃 `.litematic` 的 ArrayBuffer，不带 Mojang 资源）。维护者已选定。备选 deepslate（MIT，不读 `.litematic`）只在 V1–V5 证明 schematic-renderer 走不通时再提。
- 许可证：本项目是 AGPL-3.0-only（ADR 0025），与 AGPL-3.0 兼容。随应用发布的 JS/WASM 要带原许可证文本和来源说明（ADR 0011/0022 的做法），并确认 `attribution` 测试覆盖不到的非 Rust 文件放在哪里登记。
- gpui-wry：维护者已在本机测过能跑；本机 cargo 缓存里有 `gpui-wry 0.7.0`（Apache-2.0），与 gpui-component 0.7 同代。
- 边界（沿用插件系统的冻结决定）：plugin-api 不依赖 gpui、不出现 webview；插件的界面只来自视图树；core 不依赖 UI。webview 只存在于 `lumilio-ui`。
- 视图树新组件：`Model`。按 `plugin-system` 的规则，加组件要写进 `docs/design-language.md`；Litematica 是核心插件，满足条件。建议形状是 `View::Model { file: String }`，`file` 是插件 `ReadGameFiles` 授权目录里的相对路径，由宿主自己读字节（走同一套路径校验和大小上限），不把几十 MB 放进视图树。形状在 V3 之后定稿。
- 贴图：宿主从该实例所用版本的 `client.jar` 抽出 `assets/minecraft/{textures,models,blockstates}`，生成资源包 zip，放进启动器自己的缓存目录，按游戏版本缓存。游戏没装（没有 jar）时显示无贴图的占位渲染，并说明原因。

## References

- `3rd-party/` 里没有 Litematica 或 webview 相关代码；本计划不改 `3rd-party/`。
- 现有的「读 jar」代码：`crates/lumilio-core/src/layout.rs`、`release.rs`（找 `client.jar` 的位置）。
- 本机缓存：`~/.cargo/registry/src/*/gpui-wry-0.7.0`。
- 候选库调研（2026-10-04）：schematic-renderer（AGPL-3.0）、deepslate（MIT）、LitematicWebViewer / litematic-viewer / litematic-studio（无许可证声明，不能抄代码）。
- ADR 0011、0022、0025；`plugin-system` 计划的 D1、D5。

## Tasks

### 阶段 V：先验证，不写产品代码

- [ ] V1 用任务专用的数据目录和临时示例（验证代码不进产品、不提交），让 gpui-wry 在我们当前锁定的 gpui 版本下，于实例页的一个 `div` 区域内显示本地 HTML；记下它占的是原生子视图还是离屏纹理、窗口缩放和滚动时的表现。
- [ ] V2 叠层：在 webview 显示时打开确认弹窗、toast、顶部下拉菜单，看它们能不能盖在 webview 上。不能盖的话，试「弹窗打开时隐藏 webview 并显示最后一帧截图」是否可接受，写进记录。
- [ ] V3 把 schematic-renderer 的 UMD 作为本地资源加载（不联网），把一份 `.litematic` 字节从宿主传给它（自定义协议或 IPC 二选一，记下取舍），确认能出画面；记下 UMD + WASM 的体积和首次加载耗时。
- [ ] V4 从某个版本的 `client.jar` 生成资源包 zip，喂给 schematic-renderer，确认贴图、方块状态、模型都正确；对 1.21 和最新版本各试一次，记下哪些方块缺贴图。
- [ ] V5 大投影：用 10 万、100 万方块量级的投影，记下加载耗时、内存和旋转帧率；确定「太大时」的降级办法（只显示一个范围、或只提示不预览）。
- [ ] V6 Linux / Windows：确认依赖（Linux 的 webkit2gtk、Windows 的 WebView2）和 CI 构建是否需要改，写进记录；本机只能测 macOS 的部分如实标注。

### 阶段 I：实现（V1–V6 全部通过，并经维护者确认后才开始）

- [ ] I1 ADR：记录采用 webview + schematic-renderer、贴图来源、许可证处理（先写决定，再落代码；也可在完成时压缩成决策记录）。
- [ ] I2 core：`client.jar` → 资源包 zip 的生成与缓存（工作线程，缓存目录，按版本；带测试，用测试里自己造的 zip 当 jar）。
- [ ] I3 plugin-api：`View::Model`；core 的宿主按 `ReadGameFiles` 读文件，把字节和资源包交给 UI（经服务方法）。
- [ ] I4 lumilio-ui：webview 承载组件（只在显示 `Model` 时创建，离开页面销毁），叠层处理按 V2 的结论；渲染器把 `Model` 映射到它；`docs/design-language.md` 写明这个组件。
- [ ] I5 Litematica：详情加 `Model`；过大时按 V5 的结论降级。
- [ ] I6 IA 注释、`just ia`，第三方资源的许可证与来源登记。

## Validation

- 阶段 V 每一项都要有记录里的证据（耗时、体积、截图描述），不凭印象勾选。
- 阶段 I：core 的资源包生成和 `Model` 的文件读取有单元测试（含越权路径被拒绝）；UI 只测映射和销毁/创建时机；每次提交过 `just check`。
- 需要维护者肉眼看：贴图是否正确、旋转缩放手感、弹窗与 webview 的叠放、深色模式下的边框。

## Open questions

- `Model` 在没有 `client.jar` 时，是占位渲染，还是干脆不显示预览？（V4 后定）
- webview 的 JS 资源放在 `lumilio-ui/assets/` 里随二进制打包，还是首次使用时下载？倾向打包（离线可用），但 UMD + WASM 体积由 V3 决定。
- 弹窗叠层不可解时的退路是否可接受，由维护者在 V2 后裁决。

## 实施记录

### 2026-10-04 — V1/V2 结论（只读源码，未运行）

- 来源：`gpui-wry-0.7.0` 的 README 与 `src/lib.rs`（全文 247 行）。
- V1：维护者已在本机测过能跑，本计划采信；我没有重复。依赖锁定 `gpui-pre =0.3.7`、`lb-wry 0.53.3`，与当前工作区一致。
- V2（源码结论，非实测）：README 明说「WebView 渲染在 GPUI 窗口之上，webview 范围内的 GPUI 元素都会被盖住」，并建议放进单独窗口或弹出层。实现上它是原生子视图，每帧在 prepaint 里 `set_bounds`，不受 GPUI 的裁剪和层级约束。对实例页意味着：
  - 页面滚动时，webview 会滚到顶部标题栏、底部导航岛和浮动工具条的上面；
  - 确认弹窗、toast、菜单都盖不住它；
  - 唯一的缓解是弹窗时 `hide()`，但滚动穿透导航岛的问题没有简单办法。
- V6（部分）：README 写明「目前只支持 macOS 和 Windows」，没有 Linux。CI 只跑 macOS（`ci.yml`），所以现状不受影响，但 Linux 将没有 3D 预览，需要降级。
- 由此提出调整（待维护者裁决）：不把 webview 嵌进会滚动的实例页，而是「打开 3D 预览」时开一个**单独窗口**（`gpui_kit::open_window`），窗口里整片都是 webview。这样没有叠层、滚动、弹窗问题，可以放大、可以自由缩放。`View::Model` 在页面里渲染成一块带「3D 预览」键的占位卡片。代价：预览不在页面内联，且仍然只有 macOS / Windows。
