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

- [x] V1 用任务专用的数据目录和临时示例（验证代码不进产品、不提交），让 gpui-wry 在我们当前锁定的 gpui 版本下，于实例页的一个 `div` 区域内显示本地 HTML；记下它占的是原生子视图还是离屏纹理、窗口缩放和滚动时的表现。
- [x] V2 叠层：在 webview 显示时打开确认弹窗、toast、顶部下拉菜单，看它们能不能盖在 webview 上。不能盖的话，试「弹窗打开时隐藏 webview 并显示最后一帧截图」是否可接受，写进记录。
- [x] V3 把 schematic-renderer 的 UMD 作为本地资源加载（不联网），把一份 `.litematic` 字节从宿主传给它（自定义协议或 IPC 二选一，记下取舍），确认能出画面；记下 UMD + WASM 的体积和首次加载耗时。
- [x] V4 从某个版本的 `client.jar` 生成资源包 zip，喂给 schematic-renderer，确认贴图、方块状态、模型都正确；对 1.21 和最新版本各试一次，记下哪些方块缺贴图。
- [x] V5 大投影：用 10 万、100 万方块量级的投影，记下加载耗时、内存和旋转帧率；确定「太大时」的降级办法（只显示一个范围、或只提示不预览）。
- [x] V6 Linux / Windows：确认依赖（Linux 的 webkit2gtk、Windows 的 WebView2）和 CI 构建是否需要改，写进记录；本机只能测 macOS 的部分如实标注。（只验证了 macOS；Linux 不支持，Windows 未测，见实施记录）

### 阶段 I：实现（V1–V6 全部通过，并经维护者确认后才开始）

- [ ] I1 ADR：记录采用 webview + schematic-renderer、贴图来源、许可证处理（先写决定，再落代码；也可在完成时压缩成决策记录）。
- [ ] I2 core：`client.jar` → 资源包 zip 的生成（工作线程；按需生成，不预先缓存，库在 IndexedDB 里按名字缓存；带测试，用测试里自己造的 zip 当 jar）。
- [ ] I3 plugin-api：`View::Model`；core 的宿主按 `ReadGameFiles` 读文件，把字节和资源包交给 UI（经服务方法）。
- [ ] I4 lumilio-ui：单独的预览窗口（`gpui_kit::open_window`，窗口里整片是 webview，关窗销毁）；`View::Model` 在页面里渲染成带「3D 预览」键的占位卡片；`docs/design-language.md` 写明这个组件。Linux 上隐藏这个键并说明原因。
- [ ] I5 Litematica：详情加 `Model`；过大时按 V5 的结论降级。
- [ ] I6 IA 注释、`just ia`，第三方资源的许可证与来源登记。

## Validation

- 阶段 V 每一项都要有记录里的证据（耗时、体积、截图描述），不凭印象勾选。
- 阶段 I：core 的资源包生成和 `Model` 的文件读取有单元测试（含越权路径被拒绝）；UI 只测映射和销毁/创建时机；每次提交过 `just check`。
- 需要维护者肉眼看：贴图是否正确、旋转缩放手感、弹窗与 webview 的叠放、深色模式下的边框。

## Open questions

- `Model` 在没有 `client.jar` 时，是占位渲染，还是干脆不显示预览？（V4 后定）
- JS/WASM 资源随二进制打包（离线可用）：实际需要的文件原始 25 MB，压缩后约 6 MB（见实施记录）。是否进一步用打包工具裁剪（去掉没用到的导出器/Inspector）？
- 投影用的方块名与实例版本不一致时（如 `chain` 在新版本叫 `iron_chain`）会缺方块。按投影的 `MinecraftDataVersion` 选资源包，还是只在界面上提示？
- 更老的版本（尤其 1.13 之前）没有测过，最老支持到哪个版本？
- 预览窗口的默认镜头比较小（模型只占画面一部分），需要在 I4 里调镜头参数。

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

### 2026-10-04 — V3–V6 结论（实测，macOS，WKWebView）

验证用的临时程序在任务临时目录里（tao + lb-wry 0.53.3，自定义协议 `lumilio://`），不进产品、不提交。真实投影是一份 13 KB 的 `.litematic`；规模测试用脚本合成的投影（地形加少量方块，格式按 Litematica 的布局打包）。页面把画布导出成 PNG 回传，我看过画面，不只是看日志。

- **V3 能跑，但不是 README 说的「单文件 UMD」**。`schematic-renderer@1.6.1` 的 UMD 把 `three`（全局 `THREE`）和 `nucleation`（全局 `Nucleation`）当外部依赖，直接加载会在 `new N.Ray` 处报错。可用的做法是 **ES 版本 + importmap**：`three` 指向本地 `three@0.184.0` 的 `three.module.js`/`three.core.js`，`nucleation` 指向本地 `nucleation@0.2.18`（注意是 0.2.x；npm 上最新的 0.10.x 是完全不同的 API）。库的其余部分是动态 `import()` 的分块文件，由自定义协议按路径提供即可。自定义协议需要返回正确的 MIME（根路径 `/` 也要是 `text/html`，`.wasm` 为 `application/wasm`）。
- 自定义协议下 `crossOriginIsolated` 为 true，但 WKWebView 里 `SharedArrayBuffer` 不可用；库有回退，不影响出图。WebGL2 可用。
- **加载顺序**：必须等 `onRendererInitialized` 之后再调 `schematicManager.loadSchematic(id, arrayBuffer)`，否则报错（错误对象没有堆栈）。投影字节用 `fetch('/schematic')` 从自定义协议取，宿主不需要把文件内容塞进页面。
- **体积**：实际会被请求的文件原始 25.0 MB（`schematic-renderer.es.js` 11.7、`nucleation_bg.wasm` 10.2、three 2.1、其余约 1），zlib-9 压缩后约 5.8 MB。发布包里的 UMD（13 MB）、`pack.zip`、`schematics/`、`.d.ts` 都不需要。耗时：模块加载约 50 ms，渲染器初始化约 200 ms。
- **`dist/pack.zip` 是 Mojang 的默认贴图**（`pack.mcmeta` 写着 "Minecraft's Default Textures for Pack & Mod Creators"）。绝不能随应用发布，也不要把它拷进仓库。库本身没有内嵌贴图：无资源包时只画出自带的几个箱子模型。
- **V4 用 `client.jar` 生成的资源包可用**。只取 `pack.mcmeta` 加 `assets/minecraft/{blockstates,models,textures,atlases,items}`，不压缩（stored）写成 zip：26.2 为 6.8 MB、1.21.11 为 6.3 MB，生成约 0.3 秒；渲染结果与默认包一致（深板岩砖、草地、竹板等都对）。26.3、1.21.11 之外的版本本机没有可用的原版 jar，未测。已知缺口：投影里的旧方块名 `minecraft:chain` 在 1.21.11 和 26.2 的资源里都找不到定义（被改名了），渲染时该方块缺失，库打出 warn 但不崩。
- **资源包会持久化到 IndexedDB，而且必须持久化**。库的资源包管理器在 IndexedDB 里按名字存包（`cubane-resource-packs` 等）；`defaultResourcePacks` 的加载函数只在该名字不存在时才调用。证据：无痕数据存储下加载失败（`Failed to fetch default pack`），画面几乎为空；非无痕时第二次运行不再请求 `/pack.zip`。设计后果：
  1. 资源包名要带游戏版本（以及 jar 的校验值），比如 `jar-26.2-<hash>`，否则换版本会读到上一个版本的缓存（我自己在验证中就踩过：名字一直是 `vanilla`，后面的「26.2 包」其实是第一次存的 npm 包）。
  2. 资源包 zip 可以在自定义协议收到 `/pack.zip` 请求时再按需生成，不用预先缓存；缓存命中时根本不会请求。
  3. 要定期清理不再使用的包（库有 `removePack`/`removeAllPacks`）。
  4. webview 必须用持久数据存储，不能无痕；macOS 上可用 wry 的 data store identifier 把它和别的页面隔开（本次未验证，I4 再确认）。
- 冷启动（包不在库里）渲染器初始化约 700–750 ms，其中处理 6.8 MB 的包约 550 ms；热启动约 200 ms。
- **V5 性能（60 fps 是 rAF 在自动旋转时的实测，窗口 900×640 逻辑像素）**：

  | 投影 | 体积 / 实心方块 | 调用到渲染完成 | 帧率 | WebContent 内存 |
  |---|---|---|---|---|
  | 真实 13 KB 小房子 | — | 约 0.6 s | 60 | 约 650 MB（基线） |
  | 合成 | 10 万 / 5.2 万 | 约 1.0 s | 60 | 约 600 MB |
  | 合成 | 100 万 / 50 万 | 约 0.6 s | 60 | — |
  | 合成稠密棋盘 | 100 万 / 50 万 | 约 2.2 s | 60 | 约 1.2 GB |
  | 合成 | 800 万 / 400 万 | 约 2.4 s | 60 | 约 840 MB |
  | 合成 | 2700 万 / 1350 万 | 约 7.0 s | 60 | 约 1.6 GB |

  内存里约 600 MB 是 WebKit、WASM 和资源包的固定开销。结论：绝大多数投影（宿主读文件上限 32 MiB）都在几秒内出图，不需要降级预览；只需要在加载超过一两秒时显示进度，并在窗口关闭时释放。
- **V6**：只验证了 macOS。`gpui-wry` 不支持 Linux（README），Linux 上没有 3D 预览，需要隐藏入口并说明；CI 只跑 macOS，所以不影响现状。Windows 没测：WebView2 的自定义协议地址形式不同（`http://<scheme>.localhost`），`gpui-wry` 声明支持 Windows，实现时再验证并如实标注。
- 镜头：默认镜头偏小、偏远（模型只占画面中间一小块）；库有 `cameraOptions.useTightBounds`，I4 调。
- 许可证：schematic-renderer AGPL-3.0-only，three.js MIT，nucleation MIT；随应用发布时三者的许可证文本都要带上。
