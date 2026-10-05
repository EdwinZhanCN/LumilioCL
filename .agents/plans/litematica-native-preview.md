# Litematica 3D 预览改为原生渲染

- Status: proposed

## Goal

投影详情页里内联显示 3D 预览，可以拖拽旋转、滚轮缩放，画面用游戏自己的贴图。全部由 Rust 完成：Nucleation 解析并生成网格，wgpu 离屏渲染，回读成图片交给 gpui 显示。不再有 webview、JS、WASM，也不再开单独的窗口；macOS、Windows、Linux 同一套代码。

这个计划取代 ADR 0027 里「单独 webview 窗口」的决定（0027 的其余部分，如 `View::Model`、由 `client.jar` 生成资源包，继续有效）。

## Scope

- In：
  - 新 crate：持有 wgpu 设备的离屏渲染，不依赖 gpui。
  - 页面内联的 `Model` 组件：拖拽、缩放、复位、加载中和出错状态。
  - 撤掉 webview 方案的全部代码和资源。
  - 没有可用 GPU 时降级成一句说明。
- Out：
  - 皮肤 3D 预览、编辑投影、实体渲染。
  - 官网那样的 SSAO、全局光照、调色（先用 Nucleation 自带的方向光和环境光）。
  - 其他格式（`.schem`、`.nbt`）：Nucleation 能读，但本计划只接 `.litematic`。
  - 等 gpui 提供自定义 GPU 纹理接口后的零拷贝：以后只替换「回读成图」这最后一步。

## 已验证的事实（2026-10-04，macOS，Apple Silicon）

- 依赖：用 git 依赖 `nucleation`（`https://github.com/Schem-at/Nucleation`，rev `51de345`，`default-features = false`，特性 `meshing` + `rendering`）。**不要用 crates.io 的 0.10.24**：它写的是 `schematic-mesher = "0.2.0"` 加 git，发布时 git 半边被丢掉，解析到旧版网格层，`animation/glb.rs` 编译失败。网格层 `Schematic-Mesher` 锁在 rev `286323e`（由 Nucleation 的 Cargo.toml 指定）。
- 原生渲染正确：真实投影（3 个子区，约 7000 个方块）完整，玻璃半透明可见；多子区由 Nucleation 自己处理，不需要我们合并。
- 成本（release）：真实文件解析 7 ms、网格 68 ms、创建渲染器 13 ms，之后每帧渲染加回读 1.1 ms（1800×1200）。合成 2700 万体积（贪心合并）网格 0.86 s、每帧 4 ms。
- gpui 整条链（debug 构建）：不含红蓝交换时，1000×700 和 2000×1400 都是约 117 fps，3000 帧后内存与 600 帧时相同（294 MB），`window.drop_image` 有效。RGBA→BGRA 的逐像素循环在 debug 下很慢（8–31 fps），release 下应为毫秒级，需实现时实测。
- 冷编译：release 51 秒；debug 示例约 21 分钟（wgpu 等新依赖）。
- 26.2 的新格式有几处识别不了：告示牌、床的贴图缺失（粉红），一个 `template_hanging_sign` 模型解析告警；1.21.11 没有这个问题。

## 设计

- 新 crate `lumilio-schematic-render`（名字实施时可改）：只依赖 `nucleation`、`pollster`，不依赖 gpui、core、ui、app。
  - `Scene::load(schematic_bytes, pack_bytes)`：解析、生成网格、创建 `GpuRenderer`。阻塞，在工作线程里调用。
  - `Scene::render(&View, width, height) -> Vec<u8>`：返回 BGRA。`View` 是我们自己的相机参数（偏航、俯仰、距离倍率、目标点），由本 crate 转成 Nucleation 的 `CameraConfig`。
  - 错误：`NoGpu`（没有适配器）、`Parse`、`Pack`、`Mesh` 等，各有一句人话。
- `GpuRenderer` 不一定 `Send`：由一个专用工作线程持有 `Scene`，UI 通过通道发「相机 + 尺寸」请求，收回帧；请求合并，只渲染最新的一个。
- UI 的 `ModelView`：持有这个线程的句柄，页面内联显示最新一帧（`img(Arc<RenderImage>)`），每换一帧就对旧图调用 `window.drop_image`。
  - 按需渲染：静止时不出帧，只在拖拽、缩放、窗口尺寸变化时请求。
  - 以物理像素尺寸渲染（Retina 下是逻辑尺寸的两倍）。
  - 拖拽改偏航和俯仰，滚轮改距离，双击或按 R 复位。滚轮必须留在组件里：按「`keep_wheel`」的做法处理，不让页面跟着滚。
  - 有动画方块时再按 20 fps 重绘（先做静态，动画作为后续任务）。
- 数据流不变：插件的视图里写 `View::Model { file }`，宿主按 `ReadGameFiles` 读文件，服务方法 `plugin_model` 返回投影字节和从 `client.jar` 生成的资源包；游戏没安装（没有资源包）就不能预览，直接提示。

## References

- Nucleation（MIT）：`src/rendering/{mod,gpu,camera}.rs`，`docs/features/meshing-and-rendering.md`，`examples/readme/meshing-and-rendering/rust/src/main.rs`。
- Schematic-Mesher（AGPL-3.0-only）：`src/mesher/{liquid.rs,entity/chest.rs}`。
- ADR 0011、0025、0027；`plugin-system` 计划的 D1、D5；记忆里的「嵌套滚动容器」做法。
- 本机验证用的临时程序不在仓库里；结论都写在上面。

## Tasks

- [ ] T1 ADR 0028：采用原生渲染，取代 ADR 0027 的 webview 窗口；记录依赖（git 依赖和原因）、许可证、已验证的事实和已知限制。
- [ ] T2 新建渲染 crate：`Scene::load` / `Scene::render`、相机参数、错误类型。测试：相机数学和错误转换用单元测试；渲染本身的测试用 Nucleation 的 `Schematic::create` 造一个小场景加测试里造的最小资源包，没有 GPU 适配器时跳过并说明。
- [ ] T3 BGRA：渲染 crate 里把 Nucleation 的 RGBA 换成 BGRA，写成能被编译器向量化的形式并用 release 实测；如果仍然是瓶颈，再考虑改上游或换种输出。
- [ ] T4 `lumilio-core`：保留 `model_assets`（资源包）、`plugin_model`、`read_model`；去掉资源包 id。`model` 钩子和合并子区代码（提交 `6d5d591`）整体撤销。
- [ ] T5 `lumilio-ui`：`ModelView`（工作线程、请求合并、`drop_image`、拖拽和缩放、复位、加载中和出错状态）；`View::Model` 在详情页里内联渲染成它；`docs/design-language.md` 写明这个组件。
- [ ] T6 撤掉 webview 方案：`model_preview` 模块、`assets/litematic-viewer/`、`gpui-wry` 和 `wry` 依赖、app 里打开窗口的代码、`InstanceIntent::PluginModel`、`Cargo.lock` 里相关条目。
- [ ] T7 许可证与来源：`ATTRIBUTIONS.md` 登记 Nucleation（MIT）和 Schematic-Mesher（AGPL-3.0-only）、锁定的 rev。
- [ ] T8 没有 GPU 时的降级：`NoGpu` 显示一句说明，不崩；其他错误同理（贴图缺失只是视觉问题，不是错误）。
- [ ] T9 IA 注释和 `just ia`；`just check`。
- [ ] T10 用真实投影在原生窗口里验证：多子区、玻璃、大箱子、流体，拖拽和缩放的手感，滚轮不带动页面，窗口缩放，深浅色。

## Validation

- 渲染 crate：单元测试加（有 GPU 时）一个渲染测试；性能数字（release）写进实施记录：加载耗时、每帧耗时、BGRA 一步的耗时。
- UI：测试 `ModelView` 的状态流转（加载中、出错、有帧）、请求合并、拖拽和缩放对相机参数的影响、`NoGpu` 的显示；每次提交过 `just check`。
- 需要维护者肉眼看：贴图和形状是否正确、手感、Retina 下是否清晰、窗口缩放。

## Open questions

- 实例页是页面滚动的，内联组件的高度怎么定（固定高度？跟窗口高度？）；需要看真实页面再定。
- 有动画方块（水、岩浆、火）时是否要动起来，还是保持静态。
- 26.2 的告示牌、床缺贴图：等 Nucleation 更新，还是我们在资源包里补？
- Windows、Linux 没有实测：Linux 没有 Vulkan 时会走 `NoGpu`，需要在实现后找环境试一次，并如实标注。
- 渲染 crate 依赖 git 版本的 `nucleation` 和 `schematic-mesher`：CI 构建时要能访问 GitHub；是否要把它们 vendor 进仓库，由维护者定。
