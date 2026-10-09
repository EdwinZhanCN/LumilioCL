# World Explorer：原生 2D 世界地图

- Status: in_progress

## Goal

游戏页多一个「地图」标签。它只有一个视口和一个当前底图，底图在种子预测（cubiomes）、真实存档（Anvil）和 Xaero 世界地图之间单选；上面可以同时开任意多个 Overlay：cubiomes 结构、史莱姆区块和出生点，Xaero 路径点，存档里的出生点，启动器自己的标记、路线和测距，以及区块网格、Region 边界和坐标。人可以平移、缩放、切维度、跳到坐标、点一个对象复制坐标；游戏没在运行时，可以在地图上编辑 Xaero 路径点并安全写回。所有计算都在本机后台进行，拖动和缩放不卡界面，过期结果不会画进新视图。种子预测先覆盖 cubiomes 原生支持的版本（到 1.21.4）；新版本（第一个目标 26.3）在功能做完之后，于延后阶段 P8 在自己维护的 cubiomes fork 里补上。地图引擎归宿主；数据源是核心插件 `lumilio.world-explorer` 里的 Provider，以后第三方 WASM 插件也能经同一套接口提供底图和 Overlay。

本计划合并并替换了 `xaero-waypoints` 和 `cubiomes-seed-map` 两份旧计划，见「从旧计划迁移」。

## Scope

- In：Java 版；三种底图与单选切换；统一 Overlay（图标、点、线、区域、文本、热力图）及宿主的绘制、命中测试、选择与优先级；World Context；2D 正交相机、瓦片、LOD、可见区域调度、取消和过期丢弃；wgpu 离屏合成；瓦片磁盘缓存与存档变化后的增量更新；按版本生成并入库的方块颜色表；Xaero 路径点的读取、分享串、编辑、新建、删除和安全写回；自定义标记、路线、测距；下界 8:1 换算工具；面向插件的 `BaseMapProvider` / `OverlayProvider` 扩展点，以及与 WASM 计划的对接。
- In（延后 / 可选，P8，不阻塞收尾）：在 `forks/cubiomes` 里补新版本（1.21.5 … 26.x）的世界生成，首个目标 26.3。
- Out（产品边界，来自提案）：3D 世界地图；A/B 分屏对照；滑动差异比较；自动地形差异分析；默认把不同底图混成一张图。
- Out（工程边界）：游戏页之外的地图入口（W16）；把版本表里没有的版本映射到相邻或最新的生成实现（W14）；基岩版；种子筛选与种子搜索（cubiomes-viewer / Chunkbase 那一整套条件）；游戏内传送；在游戏运行中写任何文件；VoxelMap / JourneyMap 原生格式的读写与导入；死亡点的批量清空；Xaero 小地图在游戏内的渲染；把 cubiomes 链进 `lumilio-core`；让插件拿到 GPUI 窗口、wgpu Device、绘图原语或任意文件；云服务。

## 接手须知

- 先读：本计划；ADR 0031（插件宿主 D1–D7）、0028/0029/0036（离屏 wgpu、forks、弹窗相机与请求身份）、0011/0022/0025（派生代码与许可）、0019（IA 路径）、0026（检查）；`.agents/plans/wasm-plugin-registry.md`（P7 依赖它）；`forks/README.md` 和 `forks/cubiomes/LUMILIO.md`（P0 新建，fork 的本地补丁日志，W14）；skills `lumilio-i18n`、`lumilio-ia-paths`、`lumilio-write-a-test`、`gpui-kit`、`gpui-kit-design-guides`、`lumilio-motion-design`。
- 参照代码：插件写法看 `crates/lumilio-plugin-litematica/src/{lib.rs,tab.rs}`；宿主看 `crates/lumilio-core/src/plugins/{mod.rs,access.rs,context.rs,tabs.rs,model.rs}`；离屏渲染与 GPUI 拼接看 `crates/lumilio-schematic-render/src/scene.rs` 和 `crates/lumilio-ui/src/model_view/{mod.rs,worker.rs,modal.rs}`；插件视图渲染器是 `crates/lumilio-ui/src/instance_detail/plugin_tabs.rs`；插件注册在 `crates/lumilio-app/src/backend.rs` 的 `Backend::open`。
- 容易踩的坑（都已在代码里核实）：
  - 现有插件调用默认 5 秒超时，panic、超时和越权会把插件打成本次运行粘住的 `Failed`（`plugins/mod.rs` 的 `isolated` / `call_scoped`）。瓦片调用不能直接走这条路，见 W15。
  - `forks/cubiomes` 是我们自己维护的世界生成实现，不是只读的参考。新版本的支持按 W14 的流程加：先有差异清单和实测金样，最后才进版本表。不要为了让某个版本「能看」而把它映射到相邻版本。新版本支持在 P8，延后，不要在功能阶段里顺手开工。
  - 从 Mojang 资料派生的数据（数值表、报告摘录、方块颜色表）可以入库，但每份都要带来源记录，能重新生成（W13、W14）。jar 本身和反编译 / 反混淆的源码不入库。
  - `catch_unwind` 接不住 C 代码的段错误或 `abort`。cubiomes 的输入必须在 Rust 侧先校验（W11）。
  - 插件读文件上限 32 MiB（`access.rs` 的 `MAX_FILE_BYTES`），`list_files` 递归、最多 5000 项、深度 8。Region 文件和 Xaero 世界地图目录都会撞上这些限制，P4/P5 需要新的宿主文件接口。
  - `TabState` 只在内存里（`PluginHost::tab_states`），重启就没了。需要保存的东西（世界关联、标记）归宿主持久化。
  - 视图树 D5 没有任何输入控件，只有 Toggle/Choice/Text/Number 在 D6 设置表单里。旧计划说「视图树适合列表和表单」不对；编辑路径点要新的声明式编辑契约（W10）。
  - 游戏运行时 `launch_with` 一直持有实例租约（`service/launch.rs` 调 `reserve_instance`），所以写回在 core 取同一个租约，拿不到就是在运行（W7）。别的启动器开着同一个目录时租约看不见，靠冲突保护兜底。
  - 工作区 `unsafe_code = "forbid"`。FFI 只能放在单独的 crate，仿照 `crates/lumilio-pointer/Cargo.toml`：crate 级 `deny`，只在 FFI 模块里 `#![allow(unsafe_code)]`。
  - 插件 crate 里的中文（清单名、视图文字）目前不进翻译目录（`hardcoded_chinese_only_shrinks` 只扫 `lumilio-ui` 和 `lumilio-app`）。地图界面由宿主画，文案全部走 `tr!`；Provider 返回类型化的种类 id，由宿主翻译（W12）。
  - 发布包只带 `LICENSE`（`crates/lumilio-xtask/src/{macos,windows,linux}.rs`），不带 `ATTRIBUTIONS.md`。cubiomes 是 MIT，要求随副本附带声明；这个缺口对已经链接的 Nucleation（MIT）同样存在。
  - `3rd-party/` 被 git 忽略，这份检出里没有。本计划引用的上游文件都给了 URL 和提交号；需要本地阅读时按 ADR 0022 的方式浅克隆到 `3rd-party/` 或 `/tmp`，不要改。
  - 旧计划里的几条事实被这次调研推翻：cubiomes 是 MIT，不是 GPL，上游只支持到 1.21.4；Xaero 路径点文件名是 `mw$default_1.txt`；名称里的冒号写成 `§§`；颜色下标有 16 和 20 两种说法。见「调研」。
- 检查：改文档跑 `just docs`；迭代时 `just test-pkg <crate> <过滤>`（例如 `just test-pkg lumilio-plugin-world-explorer xaero`）；改 UI 后 `just ia`；交付前 `just check`。CI 只在 macOS 上跑 `just ci`（`.github/workflows/ci.yml`）；Windows 和 Linux 只在 `release.yml` 的打包矩阵里编译。

## 冻结的决策

- **W1 World Context 归宿主。** 新类型 `WorldContext`（`lumilio-plugin-api` 的新模块 `map.rs`，只用 serde）：
  - `world: WorldId`，枚举 `Save { instance, folder }`、`Server { instance, address }`（只有 Xaero 数据的多人世界）、`Seed { seed, version }`（手动种子，没有存档）；
  - `version: Option<String>` 与 `data_version: Option<i32>`，来自 `level.dat` 的 `Data.Version.Name` / `Data.DataVersion`；
  - `seed: Option<i64>`，可空；来源记为 `level.dat`、`world_gen_settings.dat` 或手动；
  - `dimension: Dimension`，`Overworld | Nether | End | Custom(String)`，后者是资源 id；
  - `sources: Vec<SourceLink>`：存档目录、Xaero 小地图目录名、Xaero 世界地图目录名、种子来源。
  core 新模块 `crates/lumilio-core/src/world_map/context.rs`（新建）负责解析：在 `worlds::scan` 的基础上读种子、数据版本和出生点；列出 `xaero/minimap/*` 和 `xaero/world-map/*` 的目录名，只在名称完全相同（单人世界目录名）时提出关联建议。关联要人确认，确认后保存（W6）。不同来源的数据不会自动合并：每个空间对象都带 `source`（provider id）、`raw_id`（来源里的原始标识）、所属世界和维度。只有明确关联到同一个世界的数据源，才在同一张图上一起显示。理由：Xaero 的多人目录、改过名的存档和手动种子都无法可靠地自动对上；猜错会把别的世界的点画上来。
- **W2 `BaseMapProvider`，单选。** 插件 API 新增 trait：`fn base_maps(&self) -> Vec<BaseMapInfo>`（id、种类 id、支持的维度与 LOD 级别）和 `fn tile(&self, ctx: &dyn HostContext, request: &TileRequest) -> Result<TileReply, PluginError>`。`TileRequest` 带 `WorldContext`、维度、`TileKey`、像素尺寸（固定 256）。`TileReply` 是 `Image(ImageData)`、`Partial { image, coverage }`（局部缺数据）或 `Empty`（这一块没有数据）。同一时刻只有一个底图。切换底图时保留中心、缩放、维度和全部 Overlay 的开关。没有数据的区域画宿主的「未加载 / 无数据」底纹，并在状态行说明；绝不悄悄换成另一个底图补洞。理由：提案 §2；混合底图会让人误以为看到的是真实地形。
- **W3 `OverlayProvider`，宿主统一绘制。** 新 trait：`fn overlays(&self) -> Vec<OverlayInfo>` 和 `fn objects(&self, ctx, request: &OverlayRequest) -> Result<Vec<MapObject>, PluginError>`，请求带 World Context、维度、方块坐标范围和 LOD。`MapObject` 有 `id`、`raw_id`、`kind`、`label`（用户数据原文，例如路径点名）、`label_id`（宿主目录里的种类 id，例如 `map-structure-village`）、`priority`、`approximate`、可选的 `editable`（W10）。`kind` 是 `Icon { icon: MapIcon, at }`、`Point`、`Polyline`、`Area`、`Text`、`Heat { cell, values }`。图标是宿主自带的一套 `MapIcon` 枚举，插件不交图片；颜色只允许数据颜色（例如 Xaero 的颜色下标解析出的 RGB），线宽、字体、光晕和选中样式由宿主决定，与 D5「插件不给布局和样式」一致。命中测试、选择、标签避让和显示优先级都在宿主。理由：提案 §3；一个视口里多个来源要有同一套交互。
- **W4 Map Scene。**
  - 坐标：Minecraft 方块 X/Z，`f64`；屏幕向右为 +X，向下为 +Z（游戏里南方）。
  - 相机：2D 正交，中心点加每像素方块数。缩放连续，取数据时按 LOD 取整。
  - LOD：`level 0..=4` 对应每像素 1、4、16、64、256 方块，正好是 cubiomes `Range.scale` 支持的五档（`generator.h`）。种子底图在每一级都能直接按原生比例计算，不在 C 侧重采样。存档和 Xaero 底图只在 level 0 有原生数据，粗一级的瓦片由宿主从细一级合成并缓存（XaeroTools `pyramid.rs` 的做法）。相邻两级之间的缩放由 GPU 采样补足。每级 4 倍，不用 2 倍（维护者 2026-10-08 在 PR #6 上决定（采用默认））。
  - 瓦片：256×256 像素。`TileKey = (provider, base_map, world, dimension, level, tx, tz)`，level 0 的一块覆盖 256 方块，正好是半个 Region。
  - 调度：每次相机或数据源变化，`generation` 加一。可见集合加一圈预取，按离中心的距离排序；同时在跑的瓦片任务有上限（`available_parallelism` 减一）。离开可见集合的任务发取消；结果回来时 `generation` 或 key 不在当前集合里就丢掉。每个视口实例有自己的请求身份，关掉再开，迟到的结果进不了新视口（ADR 0036 的做法）。
- **W5 渲染器复用离屏 wgpu。** 新 crate `lumilio-map-render`（新建）只依赖 `wgpu` 和 `pollster`，不依赖 GPUI 和 launcher，和 `lumilio-schematic-render` 一样输出 BGRA 帧。`wgpu` 用 `forks/nucleation` 已锁定的 30.x，不引入第三个版本（`Cargo.lock` 里已有 29.0.4（GPUI）和 30.0.1（Nucleation））。它负责瓦片纹理缓存（按字节 LRU）、实例化绘制瓦片四边形，以及批量绘制点、线、面、热力图和网格。文本标签、选中光晕、控件和图例由 GPUI 叠在帧上面画，因为 wgpu 侧没有字形栅格化。UI 侧的 worker、邮箱合并请求、帧替换时 `drop_image`，都照 `model_view/worker.rs` 写。只在相机、尺寸或数据变化时出帧，没有空转时钟（设计语言：没有为了「活着」的动画）。不引入 Canvas、WebView 或另一套 UI。理由：提案 §6，且 ADR 0028 已证明离屏 + 回读能放进页面里，和弹窗、提示、滚动共存。代价同 ADR 0028：每帧要回读一次 GPU。P0 测量帧时间；1800×1200 的大窗口拖动时每帧超过 8 毫秒，才回来讨论别的画法（例如瓦片改由 GPUI 的图片元素直接画），在此之前不改方案（维护者 2026-10-08 在 PR #6 上决定（采用默认））。
- **W6 本地优先的数据、缓存与同步。** 没有任何网络调用。
  - 种子瓦片按 `(seed, cubiomes 版本枚举, 生成参数, 维度, level, tx, tz, 配色版本)` 缓存，跨实例共享，放在 `Layout::map_cache()`（新建，`<数据根>/cache/world-map/seed/`）。
  - 存档和 Xaero 瓦片按实例和世界放在 `Layout::profile(id)/map-cache/`（新建，与 `thumbnails` 一样可以随便删，删了会重建）。
  - 每块缓存记下来源文件的长度和修改时间（Region 文件或 Xaero 区域 zip）。打开地图、视口移动到新 Region、游戏退出时重新取一次文件信息，只重建变了的 Region 及其上层合成瓦片。游戏在运行时，按低频轮询可见 Region。
  - 缓存总量默认上限 1 GiB，按最近使用淘汰；设置 › 存储加一行「清除地图缓存」（维护者 2026-10-08 在 PR #6 上决定（采用默认））。
  - 世界关联、手动种子、自定义标记和路线存在 `launcher.db` 的新表里，不用 JSON；schema 从 3 升到 4（`crates/lumilio-core/src/instance.rs` 的 `SCHEMA_VERSION`）。理由：ADR 0006 把关系型的启动器数据放进 SQLite，而标记要按世界、维度查询，还要关联别的来源（维护者 2026-10-08 在 PR #6 上决定（采用默认））。
- **W7 Xaero 写回安全。**
  - 运行中只读：写回入口在 core，先取实例租约（`reserve_instance`）。拿不到（游戏在跑或有别的操作）就不写，页面保留浏览和复制，并说明「退出游戏后再改」。Xaero 退出世界时会把内存里的路径点写回磁盘，运行中写的东西会被覆盖。
  - 冲突保护：读时记下文件长度、修改时间和 sha256。写前重新读，三者有一项不同就拒绝写入，重新加载并告诉人文件被别处改过。
  - 先备份再写：原文件复制到 `profiles/<id>/xaero-backups/<时间>/`（实例游戏目录之外，Xaero 不会读到），每个文件保留最近若干份。写入用同目录临时文件加 rename，与 Xaero 自己的 `.temp` 做法一致。
  - 原样保留：没改的行逐字节不变（保留原行文本和换行风格）；不认识的行和字段原样写回；损坏的文件只显示错误，不覆盖。
  - 窄写权限：新增 `Permission::WriteGameFiles { under }`，World Explorer 只声明 `xaero/minimap`。宿主检查路径（和 `access::resolve` 同一套规则：不许 `..`、绝对路径和符号链接），并且只接受 `mw$*.txt` 这种文件名。设置 › 插件里用一句话说明「会修改该游戏的 Xaero 路径点文件」。
  - 死亡点、临时点要显示出来；默认的删除和批量操作只作用于普通、启用、非临时的点，删死亡点单独确认。
- **W8 维度隔离。** 视口一次只显示一个维度；Provider 只拿到当前维度的请求；对象不会跨维度显示。下界 8:1 换算是一个显式工具：选中一个点或输入坐标，给出另一维度的对应坐标，可以复制或跳过去，但不会把另一维度的数据自动叠上来。
- **W9 插件集成。**
  - World Explorer 是编译进来的核心插件 `lumilio.world-explorer`（crate `lumilio-plugin-world-explorer`，新建）。它提供 `InstanceTab`「地图」，视图里放一个新节点 `View::Map`；同时实现内置的 Provider：种子底图、存档底图、Xaero 世界地图底图，以及 cubiomes 结构、Xaero 路径点、存档位置三类 Overlay。在设置 › 插件里关掉它，地图标签和它的 Provider 一起消失。它默认启用：只读本地文件，写回要人在地图上明确操作，不向第三方披露任何东西（D4 只默认关闭会披露状态的插件）（维护者 2026-10-08 在 PR #6 上决定（采用默认））。地图出现在哪里见 W16。
  - 原生插件的 `API_VERSION` 保持 1：新增的都是带默认实现的方法和新的枚举变体，现有四个插件不用改。WIT 另有自己的版本号（维护者 2026-10-08 在 PR #6 上决定（采用默认））。
  - 地图引擎属于宿主：World Context 与调度、缓存、标记存储在 core（`crates/lumilio-core/src/world_map/`，新建）；视口与图层面板在 UI（`crates/lumilio-ui/src/world_explorer/`，新建）；合成在 `lumilio-map-render`。插件不碰 GPUI 窗口、wgpu Device 或任意文件，这和提案 §7、D1、D5 一致。`View::Map` 的处理照搬 `View::Model`：插件只说「这里有张地图」，宿主决定怎么画。
  - 网格、Region 边界、坐标、自定义标记、路线和测距是宿主内部的 Overlay，按同一个 `OverlayProvider` 形状实现，不经插件调用。它们是启动器自己的数据和几何，没必要绕一圈。
  - 宿主从所有启用的插件收集 `base_maps` 和 `overlays`，所以第三方 Provider 和内置的出现在同一个图层面板里。
  - 瓦片和对象调用走新的宿主路径 `PluginHost::map_tile` / `map_objects`（`crates/lumilio-core/src/plugins/map.rs`，新建）：单独的超时（默认 10 秒）、`HostContext::cancelled()`（新的默认方法）让插件在行与行之间检查取消、并发上限。出错时怎么办见 W15。
  - 宿主为地图补充的能力：`HostContext::read_range(path, offset, len)`、`file_info(path)`（长度、修改时间）、`list_dir(dir)`（不递归、分页），都受 `ReadGameFiles` 约束；`write_file(path, bytes, expected: FileInfo)` 受 `WriteGameFiles` 约束，并做 W7 的检查。
  - 将来的第三方 Provider 经 WASM 开放（P7，依赖 `wasm-plugin-registry.md`）：WIT 里加 `base-map-provider` 和 `overlay-provider` 两个接口，类型与 `map.rs` 一一对应；瓦片以 RGBA 字节交回，大小有上限；同样受权限、超时和取消约束。Litematica 插件届时可以提供建筑位置和投影范围 Overlay，不用自己画地图。
- **W10 声明式编辑。** 可编辑的对象带 `editable: Vec<SettingField>`，直接复用 D6 的 `SettingKind`（Text、Number、Choice、Toggle）和它的 `accepts` 校验。宿主画编辑对话框，参照现有的 `crates/lumilio-ui/src/plugin_setting_dialog/`；确认后调用新方法 `OverlayProvider::apply(ctx, edit: ObjectEdit)`，`ObjectEdit` 是 `Create { at, values } | Update { id, values } | Delete { id }`。插件校验后经 `write_file` 写回。删除走宿主统一确认（D5 的破坏性操作规则）。理由：视图树没有输入控件；为一个插件加一套自由表单，会破坏 D5「插件描述内容」。
- **W11 cubiomes 只在 FFI crate 里。** 新 crate `lumilio-cubiomes`（新建）：`build.rs` 用 `cc` 编译 `forks/cubiomes/`（新建，W14 的 fork）；安全封装只暴露「版本、种子、维度、范围 → 群系 id」「结构位置与可行性」「史莱姆区块」「出生点」「要塞迭代」。进 C 之前在 Rust 侧检查：版本在支持表里、`scale` 属于 {1,4,16,64,256}、范围不越界、缓冲区大小正确。只有 `lumilio-plugin-world-explorer` 依赖它；`lumilio-core` 不依赖它，也不依赖 `cc`（docgen 测试守住）。cubiomes 跑在启动器进程内，不开子进程：C 崩溃靠这些校验和测试防住，真出现过崩溃再考虑子进程（维护者 2026-10-08 在 PR #6 上决定（采用默认））。版本表里没有的版本返回明确错误，界面显示「这一版还不能查」（W14）。
- **W12 文案。** 地图的所有界面文字（底图名、图层名、状态、错误、结构种类）在宿主，走 `tr!` 和中英两份目录，按 `lumilio-i18n` 写。Provider 只给种类 id（`map-structure-village`、`map-base-seed` 等），宿主用 `i18n::lookup` 取文案，取不到时显示 id 本身。用户数据（路径点名、标记名）原样显示，不翻译。插件清单的名称和描述仍是插件里的中文，跟着 `i18n-english.md` 的 T2c 一起处理。
- **W13 方块颜色表按版本生成并入库。** 存档底图和 Xaero 世界地图都要「方块 → 颜色」表（贴图平均色，加上草、树叶、水的生物群系着色标记）。表由我们自己的工具 `cargo xtask block-colors <版本>` 从该版本的原版客户端 jar 生成，写进 `crates/lumilio-plugin-world-explorer/data/block-colors/<版本>.json`，同目录的 `SOURCE.md` 记下版本、客户端 jar 的 sha1、命令和工具的提交，保证能重新生成。插件编译时带上这些表，不需要宿主新接口，也不依赖玩家装没装游戏。一个世界用不晚于它版本的最新那张表，比最早一张表还旧的世界用最早那张；表里没有的方块（模组方块、比表更新的方块）画成中性的「未知方块」色，并在状态行写明有多少。不再在运行时读玩家的 `client.jar`：入库的表已经覆盖原版方块，再保留一条运行时路径只为模组方块，代价不值得（模组方块的颜色往往不在原版贴图的位置，读了也不准）。fastanvil 的 `palette.tar.gz` 和 XaeroTools 的 `assets/colortable.bin` 不直接用：两个仓库的 MIT 由各自作者授予，只覆盖他们自己的工作，文件内容派生自游戏贴图，我们只核实了根目录的 LICENSE，没看到针对这两个文件的单独说明；而且它们的格式和覆盖的版本由别人决定。用自己的工具，来源和版本都清楚。
- **W14 cubiomes 是 LumilioCL 自己维护的 fork，新版本自己补，放在延后阶段。**（维护者 2026-10-08 在 PR #6 上决定）
  - 来源：`forks/cubiomes/` 是上游 <https://github.com/Cubitect/cubiomes> 在 `e61f90580cbdd883214a8054670dacae655e59c0`（2024-11-10）的源码快照，按 ADR 0029 维护：`forks/README.md` 的表加一行（上游、基线提交、MIT），上游的 `LICENSE`（MIT，Copyright (c) 2020 Cubitect）原样保留，`ATTRIBUTIONS.md` 写一节并附 MIT 全文。改过的 C 文件保留上游的版权头。fork 在 P0 建立。
  - 本地补丁日志：`forks/cubiomes/LUMILIO.md`（新建）。每个本地改动一条，写明改了哪些文件和符号、为哪个 MC 版本、证据来源（差异清单是哪两个版本、用了哪条生成命令、对应哪组金样），以及维护者实机核对的日期。`forks/README.md` 的「Local changes」只写一句话，指向这个文件。
  - 新加一个 MC 版本的流程，五步缺一不可：
    1. **差异清单。** 用该版本的原版服务端跑数据生成器（`java -DbundlerMainClass=net.minecraft.data.Main -jar server.jar --reports`；1.18 起服务端 jar 是 bundler，参数待按版本核实），同时从服务端 jar 解出 `data/minecraft/worldgen/**` 和 `data/minecraft/tags/worldgen/**`。和上一个已支持版本比较：多噪声群系参数表（`multi_noise_biome_source_parameter_list` 与报告里的 `biome_parameters`，主世界和下界）、噪声设置和密度函数、群系注册表（新增的群系）、结构集合（`structure_set`：salt、spacing、separation、频率、排除区）、结构的群系标签（`has_structure/*`）、新增的结构。用得上的摘录（例如参数表）存进 `forks/cubiomes/data/<版本>/`，附 `SOURCE.md`；结论写进 `LUMILIO.md`。
    2. **改 fork。** 新的 `MCVersion` 枚举项；新增群系的 id；参数表变了就重新生成 `tables/btree<版本>.h`；`finders.c` 的结构配置和可行群系；`biomenoise.c` 的噪声参数；新结构的 finder。资料来源：Mojang 发布的 jar、数据生成器的输出，以及 26.x 起不再混淆的客户端（ADR 0034 已读过 26.3 的 `CuboidRotation`）。读源码是为了确认行为，fork 里的 C 代码自己写，不翻译、不粘贴 Mojang 的代码。
    3. **金样。** 用该版本的原版服务端，按固定种子生成真实世界，在一组固定坐标采集群系（主世界、下界、末地，几个高度），并记录 `/locate structure` 的结果，存为金样（种子、维度、坐标、群系 id / 结构位置）。
    4. **对比。** Rust 测试（`crates/lumilio-cubiomes/src/tests.rs`）逐点比较 fork 的输出和金样，全部一致才算过。
    5. **实机核对。** 维护者在游戏里用 F3 看几个采样点的群系，并用 `/locate` 对照地图上的村庄和要塞。
    做完这五步，这个版本才进版本表（`lumilio-cubiomes/src/lib.rs`）。两个版本可以共用同一个生成实现，但前提是差异清单证明生成输入完全相同，而且每个版本都有自己的金样并且通过。
  - 入库规则：从 Mojang 资料派生的生成数据（报告摘录、参数表、btree 表、结构常量、金样）在有用时入库，每份都带来源记录（MC 版本、jar 的 sha1、命令、输入），能重新生成。服务端和客户端 jar 本身、反编译或反混淆的源码不入库，只放在被 git 忽略的 `target/worldgen/<版本>/` 下。
  - 保留规则：版本表里没有的版本，界面显示「这一版还不能查」，不降级到相邻版本，也不像 Axolotl 那样把新版本都映射到最新的枚举（issue #165 已经证明 26.2 的群系变了）。新版本发布到我们支持之间的空窗期就是「这一版还不能查」。
  - 交付：P0、P1 交付上游原生支持的版本（到 1.21.4）。新版本放在最后的延后阶段 P8：首个目标 26.3；1.21.5 到 26.2 之间的版本，只有差异清单证明它的生成输入与某个已实现的版本完全相同，并且它自己的金样通过时才顺带支持，其余不保证。26.3 之后的版本按上面的流程逐个加，每个版本一条 `LUMILIO.md` 记录。P8 不阻塞计划收尾（T45）。
  - 上游如果恢复更新：把上游的新提交合进 fork，对每个本地补丁判断上游是否已经覆盖。上游的实现能通过我们的全部金样，就采用上游的、删掉我们的补丁；两边结果不一致时，以金样和实机为准，并在 `LUMILIO.md` 写明差异。基线提交在 `forks/README.md` 里更新。
- **W15 瓦片和对象出错不让插件进 `Failed`。**（维护者 2026-10-08 在 PR #6 上决定；修订 ADR 0031 D2）经 `map_tile` / `map_objects` 的调用，无论是错误、panic 还是超时，都只让这一块瓦片或这一批对象显示「失败 · 重试」，不把插件打成 `Failed`，也不影响同一个插件的其他 Provider 和其他扩展点。同一个 Provider 在一次运行里连续出错 5 次（成功一次就清零），只停用这个 Provider 到重启，并在图层面板里说明原因；插件本身保持启用。普通扩展点（`InstanceTab`、`Analyzer` 等）仍按 D2：panic、超时、越权会进入 `Failed`。理由：一张地图会连续要很多块，一块失败就停掉整个插件，代价不成比例。这条修订在计划收尾时写进新 ADR（T45）。
- **W16 地图只在游戏页的插件标签里。**（维护者 2026-10-08 在 PR #6 上决定）地图只出现在游戏页里 `lumilio.world-explorer` 的「地图」标签中，没有全局入口、导航项或单独的窗口。只有种子、没有存档的情况（服务器、别人给的种子）也在某个游戏页的这个标签里用手动种子（T9）。理由：存档、Xaero 数据和版本都属于某个实例，放在游戏页里不用另外选实例；单独的入口要再设计一套「选哪个游戏」的流程。
- **W17 Xaero 钉一个版本，颜色下标以样本为准。**（维护者 2026-10-08 在 PR #6 上决定（采用默认））实现 P2 时钉住当时最新的 26.x 版 Xaero's Minimap，版本号写进样本目录名（`tests/data/xaero/<版本>/`）和本计划的实施记录。颜色下标以这个版本的真实样本为准：样本证明之前，取色器只开放 0–15；样本证明 16–19 有效后再开放。遇到不认识的下标原样保留，显示为最接近的颜色。
- **W18 构建与分发。**（维护者 2026-10-08 在 PR #6 上决定（采用默认））
  - `.github/workflows/ci.yml` 加一个 Windows 作业，只跑 `cargo build -p lumilio-cubiomes`，让 MSVC 的回归在 PR 上就暴露；完整的 `just ci` 仍只在 macOS 上跑（ADR 0026 不变）。
  - `cargo xtask package` 把 `ATTRIBUTIONS.md` 作为第三方声明放进三种发布包，同时补上已经链接的 Nucleation（MIT）缺的声明。

## crate 布局与依赖边界

| crate | 状态 | 可以依赖 | 不可以依赖 |
|---|---|---|---|
| `lumilio-plugin-api` | 修改：新增 `src/map.rs`，`View::Map`，`Permission::WriteGameFiles`，`HostContext` 新方法（都有默认实现） | `serde`、`serde_json`（D1 不变） | 其他一切 |
| `lumilio-cubiomes` | 新建；C 源码在 `forks/cubiomes/` | 构建依赖 `cc`（`Cargo.lock` 已有 1.4.4，来自 rusqlite 的 bundled SQLite） | core、ui、app、gpui、plugin-api |
| `lumilio-anvil` | 新建（P4）：Region 头、区块解压（gzip、zlib、无压缩、LZ4）、外置 `.mcc` 区块、打包的长整型数组、群系调色板、高度图与顶层方块 | `lumilio-nbt`、`flate2`、一个纯 Rust 的 LZ4 实现（`lz4_flex`，待核实版本） | core、ui、app、gpui |
| `lumilio-plugin-world-explorer` | 新建：清单、`InstanceTab`、各 Provider、Xaero 格式（`src/xaero/`）、入库的方块颜色表（`data/block-colors/`） | `lumilio-plugin-api`、`lumilio-nbt`、`lumilio-anvil`、`lumilio-cubiomes`、`serde_json`、`zip`、`image`（PNG） | core、ui、app、gpui、wgpu |
| `lumilio-map-render` | 新建 | `wgpu` 30（与 Nucleation 同版本）、`pollster` | gpui、core、plugin-api |
| `lumilio-core` | 修改：`src/world_map/`（新建）、`src/plugins/map.rs`（新建）、`Layout::map_cache`、schema 4 | 照旧；不加 `lumilio-cubiomes`、`cc` | ui、gpui |
| `lumilio-ui` | 修改：`src/world_explorer/`（新建）、`plugin_tabs.rs` 处理 `View::Map`、设置页权限说明 | `lumilio-map-render`（与 `lumilio-schematic-render` 同样的方式） | — |
| `lumilio-app` | 修改：`backend.rs` 注册 `WorldExplorer` | 照旧 | — |
| `lumilio-xtask` | 修改：P4 新增 `block-colors`；P8 新增 `worldgen-diff`、`worldgen-golden`、`cubiomes-btree`（开发工具，不进发布包） | `lumilio-anvil`、`lumilio-nbt`、`serde_json`、`zip`；运行时下载原版 jar 到被 git 忽略的 `target/worldgen/` | core、ui、gpui |

docgen 测试的改法（`crates/lumilio-docgen/tests/plugin_boundaries.rs`）：

- 现有的 `plugin_crates_obey_dependency_boundaries` 自动覆盖新的 `lumilio-plugin-world-explorer`，不用改。
- 新增一组「插件可用的库」检查：`lumilio-cubiomes`、`lumilio-anvil`、`lumilio-nbt` 不得依赖 core、ui、app 和 `gpui*`；它们不要求依赖 `lumilio-plugin-api`。
- 新增 `lumilio-map-render` 不得依赖 `gpui*`、`lumilio-core`、`lumilio-plugin-api`。
- 新增 `lumilio-core` 的 `[dependencies]` 和 `[build-dependencies]` 里不得出现 `lumilio-cubiomes` 和 `cc`（「cubiomes 不进 core」）。
- 新增 `lumilio-plugin-api` 只依赖 `serde` 和 `serde_json`（D1 原文，目前没有测试守着）。
- 每条新规则都按 `lumilio-write-a-test` 先做一次故意违反、看它失败。

署名测试（`crates/lumilio-docgen/tests/attribution.rs`）扩展：引用 Axolotl 文件的注释必须写 `GPL-3.0-only`；引用 cubiomes-viewer 的必须写 `GPL-3.0`；引用 cubiomes、fastanvil、XaeroTools 的必须写各自的 MIT（fastanvil 为 `MIT OR Apache-2.0`）和版权行。

## 调研

### 参考项目（2026-10-08 浅克隆核实，许可证以仓库里的 LICENSE 文件为准）

| 项目 | 地址与核实的提交 | 许可证 | 提案点名的文件 | 可以改编吗 | 归属要求 |
|---|---|---|---|---|---|
| cubiomes | <https://github.com/Cubitect/cubiomes>，`e61f90580cbdd883214a8054670dacae655e59c0`（2024-11-10，上游最后一次提交） | MIT，Copyright (c) 2020 Cubitect | 入口 `generator.h`、`biomes.h`、`finders.h`、`util.c`（`initBiomeColors`） | 可以，直接作为源码快照放进 `forks/cubiomes/` | 保留 `LICENSE`；`ATTRIBUTIONS.md` 加一节，全文附上 MIT 声明；`forks/README.md` 记基线提交和本地改动 |
| Axolotl（Modrinth App 的分支） | <https://github.com/Mystic-Stars/Axolotl>，`b957550cff0541e972435e81dd6a8693763d69b3`（2026-10-08） | `apps/app` 和 `apps/app-frontend` 都是 GPL-3.0-only（各自 `LICENSE`、`COPYING.md`）；版权：Rinth, Inc. 及原贡献者，Axolotl 改动 © 2026 Garbage Human Studio | 都存在，真实路径：`apps/app/src/seed_map/cubiomes_bridge.c`（826 行）和 `.h`、`apps/app/src/seed_map/mod.rs`、`apps/app/src/api/seed_map.rs`（Tauri 命令）、`apps/app-frontend/src/pages/LabSeedMap.vue`、`apps/app-frontend/src/lab/seed-map/features.ts`、`apps/app/src/seed_map/ores.rs`、`apps/app/build.rs`（第 148–170 行编译 cubiomes） | 可以（GPLv3 §13），按 ADR 0022 的方式：注释写源路径和 GPL-3.0-only，并改写成我们的 crate 边界 | 注释署名；`ATTRIBUTIONS.md` 一行。`apps/app-frontend/public/seed-map-assets/` 的图标部分来自 MinecraftSearch（权利归其作者，`COPYING.md` 未授权），不复制；仓库里的 Modrinth 品牌素材同样不能复制 |
| fastnbt / fastanvil | <https://github.com/owengage/fastnbt>，`986c8594aad0e9dbbe6124d2b0e60506e2125fbd`（2026-08-08） | 根目录 LICENSE 是 MIT（Copyright (c) 2020 Owen Gage）；`fastanvil/Cargo.toml` 与 `fastnbt/Cargo.toml` 声明 `MIT OR Apache-2.0` | `fastanvil/src/region.rs` 存在（609 行）；`chunk.rs` 不在根上，真实路径是 `fastanvil/src/java/chunk.rs` 和 `fastanvil/src/complete/chunk.rs`；`fastanvil/src/render.rs` 存在（`TopShadeRenderer`） | 可以改编 | 注释写源路径与 MIT 版权行；`ATTRIBUTIONS.md` 一节。`palette.tar.gz` 由 `tools/` 从游戏贴图生成，不直接用（W13） |
| XaeroTools | <https://github.com/dekrom/xaerotools>，`7bc650bdf445ec06d0d3fc0fb9e98ba4b86a6b83`（2026-09-03） | MIT，Copyright (c) 2026 Dek | 提案写的 `xaero-core/...` 实际在 `crates/xaero-core/src/`：`codec/`（`mod.rs`、`reader.rs`、`writer.rs`、`legacy.rs`、`nbt.rs`、`zipio.rs`）、`waypoints.rs`、`render/`（`mod.rs`、`colortable.rs`）都存在；`xaerotools/pyramid.rs` 实际是 `crates/xaerotools-server/src/pyramid.rs` | 可以改编 | 注释写源路径与 MIT 版权行；`ATTRIBUTIONS.md` 一节。`assets/colortable.bin`、`blockprops.bin`、`legacy_block_ids.bin` 由 `tools/xaero-colorgen` 从游戏数据生成，不直接用（W13） |
| cubiomes-viewer | <https://github.com/Cubitect/cubiomes-viewer>，`3acc863245b30c655498d60323c50da0865e0199`（2024-11-10） | 主体 GPLv3（LICENSE 全文；README 只写 GPLv3，没写 or later，按 GPL-3.0-only 对待）；README 写明生物群系配色和图标「受 Amidst 启发」，Amidst 是 GPLv3 | 提案没点名文件；只作 UI 与交互参考 | 法律上可以（GPLv3 §13），本计划只参考交互，不复制代码和图标 | 若真的改编了代码，注释写 GPL-3.0 并加 `ATTRIBUTIONS.md` 行 |
| Amidst（配色出处） | <https://github.com/toolbox4minecraft/amidst> | GPLv3（`LICENSE.txt`） | cubiomes `util.c` 第 318 行注释：配色「largely inspired by the AMIDST program」 | 只用 cubiomes 自带的色表 | `ATTRIBUTIONS.md` 注明配色出自 cubiomes、受 Amidst 启发（旧计划的要求） |

Xaero's Minimap / World Map 本身不开源（许可证未在本次核实）。不反编译、不复制它的代码；格式知识来自 XaeroTools 的实现和我们自己钉住的真实样本文件。

### cubiomes 的跨平台构建

- 版本覆盖：`biomes.h` 的 `MCVersion` 到 `MC_1_21_WD`（「Winter Drop」，即 1.21.4），`MC_NEWEST = MC_1_21`。上游最后一次提交是 2024-11-10。issue #161（要求支持 1.21.5 到 26.x）和 #165（26.2 新增硫磺洞穴群系，cubiomes 系的地图画不出来）都还开着。新版本由我们在 fork 里自己补，放在延后阶段 P8，首个目标 26.3（W14）。Axolotl 把 1.21.4–26.2 都映射到 `MC_NEWEST`（`cubiomes_bridge.c` 第 130–143 行注释说生成没变），#165 证明 26.2 并非如此，所以我们不采用这种映射。
- 上游怎么接入一个版本：每个版本是 `biomes.h` 的一个 `MCVersion` 枚举项；主世界的多噪声群系查找树放在 `tables/btree<版本>.h`（目前有 `btree18.h`、`btree19.h`、`btree192.h`、`btree20.h`、`btree21wd.h`），由 `biomenoise.c` 按版本选用；结构配置和可行群系在 `finders.c`。上游的树是用 `docs/nptree_c.py` 从 IntelliJ 调试器里导出的游戏内存结构转换的（脚本自述），不能复现。我们要写自己的生成器，从数据生成器报告里的参数表重建同一棵树（P8 T41），并先用 1.21.4 的报告重建出与 `btree21wd.h` 相同的表，以证明生成器正确。
- 编译器相关代码：只有 `rng.h` 用了 `__builtin_*` 和 `__attribute__`，都包在 `#if __GNUC__` 里，另有通用回退，`UNREACHABLE()` 在 `_MSC_VER` 下用 `__assume(0)`。没有 `__int128`，没有 SIMD 内建函数。在本机（Linux）用 gcc 14 `-std=c11 -Wvla -Wpedantic` 编译 `biomenoise biomes finders generator layers noise util quadbase` 没有警告；用 clang 19 `-U__GNUC__` 强制走非 GNU 分支，除 `quadbase.c` 外都能编译（只有 `-Wparentheses` 警告）。
- `quadbase.c` 是四联女巫小屋种子搜索，用 pthread 或 Win32 线程。我们不需要它：不带它也能链接，并在本机用 seed 262 的探针跑通了 `getStructurePos`、`isViableStructurePos` 和 `genBiomes`。去掉它就没有线程依赖。
- MSVC：上游没有 MSVC 的 CI，cubiomes-viewer 的 Windows 版用 MinGW。**MSVC 编译未核实。** P0 的第一个任务是在 `windows-2022` 上实际编译（手动触发 `release.yml` 或临时 workflow）。`-fwrapv` 只对 gcc/clang 有效，用 `flag_if_supported`（Axolotl 的做法）；上游 `9ec701d` 已经修掉 `rng.h` 里依赖有符号溢出的写法，MSVC 下是否还有依赖溢出的地方，待核实。
- macOS：clang；打包目前只出运行机的架构（`macos-26` 是 arm64，xtask 的 `macos.rs` 没有 universal 构建），C 代码没有架构相关的东西。x86_64 只有在本机交叉编译时才遇到，`cc` 会按目标传 `-arch`。
- Linux：`release.yml` 的 ubuntu:22.04 容器已装 `build-essential` 和 `clang`。
- CI 与打包：`cc` 已经因为 rusqlite 的 bundled SQLite 在三个平台上跑过，所以不需要新的工具链。`ci.yml` 只有 macOS；P0 给它加一个只编译 `lumilio-cubiomes` 的 Windows 作业（W18、T0）。`cargo xtask package` 不用改构建步骤，但要把第三方声明带进包里（W18、T11）。
- 源码放置与钉版本：`forks/cubiomes/`，按 ADR 0029 作为可编辑快照：只导入 `*.c`、`*.h`、`tables/`、`LICENSE`、`README.md`，不导入 `tests.c`、`docs/`、`quadbase.*`（如果导入就要写明不编译）。`forks/README.md` 的表加一行（上游、基线提交、MIT），本地改动逐条记在 `forks/cubiomes/LUMILIO.md`（W14），`forks/README.md` 的「Local changes」只指向它。不用 git submodule：本仓库的 forks 都是快照，而且 CI 不拉子模块。
- 许可：cubiomes 是 MIT，与 AGPL-3.0-only 兼容，只需保留版权与许可声明。旧计划写的「保留 Cubitect 的 GPL 声明」是错的。

### 种子与版本从哪来

- 种子：1.16 起在 `Data.WorldGenSettings.seed`，更早在 `Data.RandomSeed`。较新的存档把世界生成设置拆到 `<世界>/data/minecraft/world_gen_settings.dat`（Axolotl `apps/app/src/seed_map/mod.rs` 的 `read_split_world_gen_seed`；从哪个版本开始拆分，待核实）。这个文件可能是 zlib 压缩，`lumilio-nbt` 目前只认 gzip 和不压缩（`parse_maybe_gzip`），要补 zlib。
- 文字种子按 Java `String.hashCode` 转换（Axolotl `parse_seed` / `java_string_hash`）；纯数字按 i64 解析。
- 版本：用 `Data.Version.Name`，取不到时用 `Data.DataVersion` 查表。表只用来决定 cubiomes 的版本枚举，不在表里就不支持。
- 出生点：`Data.SpawnX/Y/Z`；新版本是否改成了 `spawn` 复合标签，待核实，以样本为准。1.18 之前 cubiomes 估算的出生点可能不准，因为它取决于草方块（cubiomes-viewer README「Known issues」）。存档里有实际出生点时用实际值，cubiomes 的估算标成「估计」。
- 没有存档时（服务器、只有种子），页面要求人粘贴种子并选版本，不去猜。

### Xaero 文件格式要点

- 目录：小地图路径点在 `<游戏>/xaero/minimap/<世界>/<维度>/mw$<集合>_1.txt`，例如 `mw$default_1.txt`；旧计划写的 `mw$default.txt` 是旧版本的名字还是写错了，待核实，两种都要能读。单人世界用世界目录名，多人用 `Multiplayer_<地址>` 一类的名字。维度目录是 `dim%0`、`dim%-1`、`dim%1`，非原版维度是 `dim%<转义后的资源 id>`。Xaero 写入时会留下 `.temp` 文件，还有 `backup`、`backup-`……这样的备份目录（XaeroTools `crates/xaero-core/src/naming.rs`）；都不当集合列出。
- 世界地图在另一棵树：`<游戏>/xaero/world-map/<世界>/<维度>/<多世界 id>/<x>_<z>.zip`。维度目录是 `null`（或 `DIM0`）、`DIM-1`、`DIM1`，资源 id 里 `:`→`$`、`/`→`%`。所以旧计划的开放问题「World Map 与小地图是否已分叉」答案是：地图数据早已分开，路径点只在小地图那棵树里。World Map 自己是否另存路径点，待核实；本计划只读写小地图的路径点，并在页面上说明。
- 行格式，XaeroTools 说是对照 `WaypointIO` 字节码核实的：`waypoint:name:initials:x:y:z:color:disabled:type:set:rotate_on_tp:tp_yaw:visibility_type:destination`。`y` 缺省时写 `~`；`type` 是 0 普通、1 死亡点、2 旧死亡点；文件里还有 `sets:` 行；旧计划提到的 `dim:` 头，待核实。
- 名称里的冒号写成 `§§`，读时还原。这回答了旧计划的开放问题：照 Xaero 的写法转义，不自创。换行在格式里无法表示，保存时拒绝。
- 长度：社区 gist 记录分享串的名称不超过 32 个字符，缩写不超过 2 个；超出时 Xaero 报错。磁盘文件是否同样限制，待核实；编辑时按这个限制校验。
- 颜色：gist 和 XaeroPlus issue #301 说是 0–15 的 Minecraft 文字颜色下标；XaeroTools 说 `WaypointColor` 枚举有 20 个（16 色之外还有 magenta、light blue、lime、pink）。可能是版本差异。以钉住版本的真实样本为准，样本证明之前只开放 0–15（W17）。
- 分享串：`xaero-waypoint:name:marker:x:y:z:color:use_yaw:yaw:dimension`，原版维度写成 `Internal-overworld-waypoints`、`Internal-the-nether-waypoints`、`Internal-the-end-waypoints`；`yaw` 的范围是 -999 到 9999。
- XaeroTools 的解析器会丢掉注释行并重写每一行，这不满足我们的「逐字节往返」。我们改编它的字段解析，但文件模型要保留每一行的原文。
- 世界地图区域文件：zip 里一个区域流，XaeroTools 能解码主版本 0–7、次版本到 8 以及更早的无版本格式（`codec/mod.rs`、`legacy.rs`），状态调色板用 NBT。读不懂的版本报「这个版本的 Xaero 地图还不支持」，不影响其他 Provider。

### 存档 Anvil：扩展 `lumilio-nbt`、新建 `lumilio-anvil`，还是用 fastanvil

| 选项 | 好处 | 问题 |
|---|---|---|
| 直接依赖 fastanvil 0.32 | 现成的 Region 读取（gzip、zlib、无压缩、LZ4）、1.18 前后两种区块格式、`TopShadeRenderer` | 要把 serde 版的 `fastnbt` 作为第二套 NBT 实现带进来；`render` 特性带 `image 0.24`，和工作区的 0.25 重复；没有外置 `.mcc` 区块（压缩字节的 128 位）；输入大小没有我们 `lumilio-nbt` 那样的上限 |
| 把 Region 塞进 `lumilio-nbt` | 少一个 crate | `lumilio-nbt` 是 core 也依赖的小库，加 LZ4 和区块模型会让 core 一起变重，也违背「一个模块只做一件事」 |
| **新建 `lumilio-anvil`（采用）** | 复用 `lumilio-nbt` 的有界解析；只做地图需要的部分；可以按 MIT 改编 fastanvil 的 `region.rs` 和 `java/section_data.rs` 的解包算法 | 自己维护格式变化；需要用真实样本钉测试 |

`lumilio-nbt` 只补两样东西：zlib 输入（`parse_zlib` 或在 `parse_maybe_gzip` 里识别 zlib 头），以及给 Region 区块用的「按字节切片解析」入口。

## 分阶段任务

顺序按代码依赖排：P0 先把宿主、视口和第一个底图打通，后面每个数据源都只是再加一个 Provider。Xaero 路径点（P2、P3）排在存档底图（P4）之前，因为文本格式小、马上有用，而写回所需的编辑契约和写权限也是 P6 标记要用的。P4 和 P5 共用方块颜色表，所以相邻。P7 依赖 WASM 计划。P8（新版本世界生成）延后到所有功能之后，是可选的，不阻塞收尾；它依赖 P4 的 `lumilio-anvil` 读金样世界。收尾任务 T45 只依赖 P0–P7。

### P0 视口、网格与种子底图

实施记录（2026-10-08）：T0 部分完成：导入精确源码快照作为构建探针的依赖，Linux GCC 14 / Clang 19 编译及探针测试通过；新增 windows-2022 的独立构建作业。当前主机不是 ubuntu:22.04，MSVC、macOS 与指定 Ubuntu 镜像的结果均为 **pending human acceptance**，等待 PR Actions；不阻塞后续 P0。MIT 声明随快照一起加入，T11 仍需补分发。

实施记录（2026-10-08）：T1 安全范围检查、26 个精确版本名称、三条精确 DataVersion 映射和原生 RGB 配色已实现；其余 DataVersion 明确 Unsupported，不做猜测。独立 C 探针生成 45 组金样（3 版本 × 3 维度 × 5 档），测试与 clippy 通过。在生成金样文件期间启动的第一次测试遇到文件尚未生成的 setup failure，已生成后重新执行。

实施记录（2026-10-08）：T2 数据契约、Provider 默认扩展点、View::Map 和取消查询已加入；API_VERSION 仍为 1。序列化往返及无效瓦片校验测试通过。UI 暂时仅接受新节点，实际视口由 T7 接入。

实施记录（2026-10-08）：T3 独立 10 秒调用、协作取消和 worker 生命周期并发上限已加入；连续五次故障仅停 Provider，成功清零。panic / PermissionDenied / timeout / cancellation / disabled 的隔离测试通过，普通 call 仍可调用。T4 开始实现宿主世界上下文、世代调度与磁盘 LRU。

实施记录（2026-10-08）：T4 世界上下文、出生点、split worldgen 与有界 zlib 读取、可见瓦片排序/预取、世代取消、1 GiB 磁盘 LRU、service API 和设置清除入口已实现。三个调度/缓存测试通过；设置视觉验收为 **pending human acceptance**。当前缓存只服务 P0 种子源，存档/Xaero 增量来源元数据留到 P4/P5。

实施记录（2026-10-08）：T5 wgpu 30 离屏瓦片四边形、纹理 LRU、网格和 BGRA 回读通过像素测试。1800×1200、单瓦片、10 帧热身后诊断平均 5.80 ms/frame（本机软件渲染，**not representative**，不是 W5 实机通过）；真实硬件的拖动帧时间和地图外观为 **pending human acceptance**。

实施记录（2026-10-08）：T6 默认启用的世界地图插件、每个实例均显示的 Map 节点、saves/xaero 只读权限和种子瓦片 Provider 已加入。每行检查取消，所有五档原生生成；未知版本返回类型化消息 id。Provider 测试、clippy 和 just docs 通过；世界列表由宿主视口统一画，避免插件重复解析上下文。

实施记录（2026-10-08）：T8 宿主 UtilityOverlay 统一几何契约已实现，区块 16 / Region 512 方块边界；负坐标 -513 到 -1 的守卫通过，光标坐标由 UI 展示。T7 正在检查，无显示器，拖动/滚轮/键盘、明暗主题、720×480 与大窗口均为 **pending human acceptance**。

实施记录（2026-10-08）：T7 View::Map 绑定独立实体、后台数据通道、合并渲染邮箱与帧序号；世界/底图/维度选择、两类网格、拖动/滚轮/键盘、坐标跳转与逐瓦片重试已实现。中英目录与 IA 已加入；cargo check -p lumilio-app 和两项 world_explorer 测试通过，包含实际 GPUI 交互。原生画面、手感、布局与主题为 **pending human acceptance**，不声称视觉通过。

实施记录（2026-10-08）：T9 手动数字/Java UTF-16 文字种子、精确版本选择、schema 4 与实例范围持久化已实现；seed store 两个测试通过，含 emoji 与 i64::MIN。SQLite instance 表整体替换不级联删除种子；实例真正移除后清理对应记录。旧版本升级时保留 schema 0 的 legacy import 分支。

实施记录（2026-10-08）：T10 Backend 注册世界地图，按插件 id 排序断言更新；七个 backend 测试通过，验证无存档 Map、手动种子重启、关闭插件后的 tab/provider 消失。额外守卫校验宿主版本选择与 Provider 版本表一致。

实施记录（2026-10-08）：T11 cubiomes MIT 全文、上游提交与 Amidst 配色来源在 T0 随源码加入；本任务补齐 macOS app Resources/DMG、Windows ZIP/Inno 安装内容、Linux tar/deb/用户安装的 ATTRIBUTIONS.md。xtask 测试验证声明全文复制与 ZIP 路径；平台安装产物实测仍为 **pending human acceptance**。

实施记录（2026-10-08）：T12 加入 FFI/NBT 库、renderer、API-only-serde 和 core-no-cubiomes/cc 守卫，递归检查重命名、目标平台和构建依赖。五条边界规则与五条署名规则逐一注入错误，十个 probe 均出现预期断言失败，再改为拒绝断言后 docgen 全部通过。

实施记录（2026-10-08）：P0 首轮完整 `just check` 通过。收尾复查补了三个回归守卫：保留仍可见的在途瓦片、不接受已离开的旧请求；不支持版本的明确拒绝不累计 Provider 故障，避免一个新版本存档阻止随后查看受支持的手动种子；worker 因失败而取消后，当前瓦片的错误仍交给视口显示。保留瓦片与版本拒绝的守卫先复现失败再修正。GPUI 实际鼠标/按键/维度分段测试通过，移除结果校验后在旧瓦片断言处失败，恢复后通过；包含重新打开实体拒绝旧回复。切换世界/维度/底图立即清除旧帧；满队列变成可重试的瓦片失败；停用来源和 NoGpu 有中英文说明，插件名称/说明/标签跟随当前语言。修正后再次执行最终闭环。

P0 任务勾选表示实现与本机自动验证完成，**不表示人工验收通过**。T0 保持部分完成。P1 及以后未开始，本计划保留用于后续阶段。

实施记录（2026-10-08）：最终 `just check` 完整通过（workspace build → test → clippy → fmt）；最后一次 clippy 报新队列错误处理的 collapsible_if，按建议合并后重跑完整闭环通过。回归探针的四次失败均为编译完成后的预期行为断言（保留瓦片、版本拒绝、GPUI 旧回复、worker 取消后的错误），修正后全套测试为绿；边界/署名的十个失败探针同样已恢复为绿。`just ia` 生成 223 条路径、18 个文件；T0 本机 GCC/Clang、NBT、Provider、renderer、core、UI、backend、xtask、docgen 定向测试均通过。普通检查跳过的帧时间诊断已单独用 `--ignored --nocapture` 执行，结果见 T5。

P0 **pending human acceptance**：Windows MSVC Actions；macOS 与指定 ubuntu:22.04 的原生构建/探针；真实地图与群系配色、负坐标网格和粗 LOD；拖动、滚轮、键盘缩放与快速切换/逐块重试的手感；中英、浅深主题、焦点与 reduced motion；720×480 和大窗口布局；真实硬件 1800×1200 拖动帧时间（8 ms 阈值）；无可用图形设备的说明；三平台实际安装包中第三方声明的安装位置。本机没有可目视的显示器，未声称任何一项视觉或实机通过。

- [ ] T0：在 `windows-2022`、`macos-latest`、`ubuntu:22.04` 上各编译一次 `lumilio-cubiomes`（先只有 `build.rs` 和一个探针测试），把 MSVC 的结果记进本计划。失败就在 `forks/cubiomes/` 里做最小修补，并记进 `forks/cubiomes/LUMILIO.md`。编译通过后，在 `.github/workflows/ci.yml` 加一个只跑 `cargo build -p lumilio-cubiomes` 的 Windows 作业（W18）。
- [x] T1：`forks/cubiomes/`（新建，快照 `e61f905`，带 `LICENSE` 和新建的补丁日志 `LUMILIO.md`，W14）和 `crates/lumilio-cubiomes/`（新建）：`build.rs` 用 `cc` 编译，不含 `quadbase.c`，`-fwrapv` 用 `flag_if_supported`，关掉上游警告；`src/ffi.rs` 是唯一允许 unsafe 的模块；`src/lib.rs` 提供安全 API 和版本表（版本字符串、数据版本 → `MCVersion`，不在表里就返回 `Unsupported`）。测试放在 `src/tests.rs`：用固定种子取群系 id，与仓库内的金样比对。金样由一个只编译 cubiomes 源码的 C 探针生成，记录生成命令。
- [x] T2：`lumilio-plugin-api/src/map.rs`（新建）：`WorldContext`、`WorldId`、`Dimension`、`SourceLink`、`TileKey`、`TileRequest`、`TileReply`、`OverlayRequest`、`MapObject`、`MapIcon`、`BaseMapInfo`、`OverlayInfo`；`BaseMapProvider`、`OverlayProvider` trait；`Plugin::base_map_provider()` / `overlay_provider()` 默认返回 `None`；`View::Map`；`HostContext::cancelled()` 默认 `false`。序列化往返测试。
- [x] T3：`crates/lumilio-core/src/plugins/map.rs`（新建）：`map_tile`、`map_objects`，单独超时、取消令牌、并发上限，故障只落到这一块（W15）。测试：panic、超时、错误、取消都不改变 `PluginStatus`；同一个 Provider 连续 5 次故障后只停用它，第 4 次后成功一次就清零；同一插件的 `InstanceTab` 不受影响；插件被关掉后不再派发。
- [x] T4：`crates/lumilio-core/src/world_map/`（新建：`mod.rs`、`context.rs`、`schedule.rs`、`cache.rs`、`tests.rs`）：读 `level.dat` 和 `world_gen_settings.dat` 的种子、版本、数据版本（`lumilio-nbt` 补 zlib）；可见集合、`generation`、排序和取消；磁盘缓存与淘汰（默认上限 1 GiB，W6）；`Layout::map_cache()`；`LauncherService` 上的地图方法（`service/world_map.rs`，新建）。测试：旧世代的结果被丢弃；同一 key 不重复计算；缓存命中；超过上限时按最近使用淘汰。设置 › 存储的「清除地图缓存」行也在这里接上。
- [x] T5：`crates/lumilio-map-render/`（新建）：设备、瓦片纹理 LRU、瓦片和网格的绘制、BGRA 回读；没有显卡时返回 `NoGpu`。GPU 像素测试照 `lumilio-schematic-render/src/tests.rs`：一块已知颜色的瓦片画在正确的屏幕位置；缩放后位置正确；回读尺寸正确。
- [x] T6：`crates/lumilio-plugin-world-explorer/`（新建）：清单（`lumilio.world-explorer`，`default_enabled: true`（W9），`api: API_VERSION` 仍是 1，`ReadGameFiles` 覆盖 `saves` 和 `xaero`）；`InstanceTab`「地图」（插件启用时每个游戏页都出现，没有存档的游戏页也要能用手动种子，W16），视图是世界列表加 `View::Map`；种子底图 Provider 按 LOD 调 `lumilio-cubiomes` 填瓦片，配色用 cubiomes 的 `initBiomeColors`。`ia[plugin.world-explorer]` 写在插件里；`crates/lumilio-docgen/src/lib.rs` 的 `PAGES` 加 `plugin.world-explorer`（「插件 · 世界地图」）。
- [x] T7：`crates/lumilio-ui/src/world_explorer/`（新建：`mod.rs`、`worker.rs`、`camera.rs`、`layers.rs`、`tests.rs`）和 `plugin_tabs.rs` 对 `View::Map` 的处理：拖动平移、滚轮和按键缩放、维度分段、底图分段（这一阶段只有种子）、图层面板（网格、Region 边界）、光标坐标、「跳到坐标」输入、无数据和失败的底纹、每块的「重试」。控件用 `lumilio-ui` 的 `Key`、`controls` 和 `kit`，不用 gpui-component 的 Button/Switch/TabBar。所有文字走 `tr!`，中英两份目录一起写。每个入口一条 `ia[...]`。
- [x] T8：宿主内部的网格、Region 边界和坐标 Overlay（`world_map/utility.rs`，新建）。测试：边界线在 512 的整数倍上；负坐标不差一。
- [x] T9：手动种子：没有存档或读不到种子时，输入种子并选择版本；存进 `launcher.db`（schema 4，`world_map/store.rs`，新建）。版本不在版本表里时写明「这一版还不能查」，不降级（W14）。这是只有种子时的唯一入口，没有全局的地图页（W16）。
- [x] T10：`crates/lumilio-app/src/backend.rs` 注册插件；`backend/tests.rs` 里的插件顺序断言加上它。
- [x] T11：许可与分发（W18）：`ATTRIBUTIONS.md` 加 cubiomes 一节（MIT 全文、基线提交、配色受 Amidst 启发）；`forks/README.md` 加一行；`crates/lumilio-xtask` 把 `ATTRIBUTIONS.md` 作为第三方声明放进三种包（这个缺口也覆盖已有的 Nucleation），`crates/lumilio-xtask/src/tests.rs` 断言包里有它。
- [x] T12：docgen 的依赖边界与署名测试按上面「crate 布局与依赖边界」扩展，每条规则先证明能失败。

### P1 cubiomes 结构、史莱姆区块与出生点

- [ ] T13：`lumilio-cubiomes` 包装 `getStructurePos`、`isViableStructurePos`、`initFirstStronghold` / `nextStronghold`、`isSlimeChunk`、`getSpawn` / `estimateSpawn`；结构种类按 `finders.h` 的 `StructureType`，每个版本只开放该版本 finder 能给出的种类。
- [ ] T14：Overlay「结构」：至少村庄、沙漠神殿、丛林神殿、要塞、海底神殿、林地府邸、下界要塞、堡垒遗迹、末地城，以及该版本支持的其他种类（古城、试炼密室、前哨站等）；按种类分组开关。1.18+ 的沙漠神殿、丛林神殿和林地府邸标成「估计」，图层面板写明原因。
- [ ] T15：Overlay「史莱姆区块」（区域填充，只在细的 LOD 显示）和「出生点」（1.18 前标「估计」；存档里有实际出生点时以 P4 的为准）。
- [ ] T16：点选一个对象，显示种类、坐标和来源；「复制坐标」复制 `x z`。测试：至少三个版本（1.16、1.18、1.21.4 或库支持的最新版）的固定种子，结构坐标与 C 探针的金样一致；生成金样的命令写进测试注释。

### P2 Xaero 路径点（只读）与分享串

- [ ] T17：`lumilio-plugin-world-explorer/src/xaero/`（新建：`waypoints.rs`、`naming.rs`、`tests.rs`）：解析集合文件，保留每一行原文、不认识的行和字段；`§§` 转义；`~` 表示没有 Y；死亡点、旧死亡点、禁用和临时点都能识别。改编自 XaeroTools `crates/xaero-core/src/{waypoints.rs,naming.rs}`，注释署名。样本：用实现时最新的 26.x 版 Xaero's Minimap（W17）导出主世界、下界、末地各一份，放在 `tests/data/xaero/<版本>/`，版本号同时记进本计划的实施记录。往返测试：读出再写回逐字节一致。
- [ ] T18：Overlay「Xaero 路径点」：按世界、维度、集合列出；颜色按下标画；死亡点用单独图标。损坏的文件、读不到的目录变成图层状态（「这份路径点文件读不了」），不让插件 `Failed`。目录结构不认识时说明只支持小地图的路径点。
- [ ] T19：世界关联：core 列出 `xaero/minimap` 和 `xaero/world-map` 的目录名；单人世界名称完全相同时提出建议，人确认后存进 `launcher.db`；多人目录（`Multiplayer_…`）作为 `WorldId::Server` 单独出现。
- [ ] T20：选中路径点后「复制分享串」，生成 `xaero-waypoint:` 格式；维度写 `Internal-…-waypoints`。测试覆盖冒号、表情、没有 Y 的点。

### P3 Xaero 路径点编辑与写回

- [ ] T21：`Permission::WriteGameFiles { under }`、`HostContext::write_file`（`crates/lumilio-core/src/plugins/access.rs` 加写的路径检查，`context.rs` 实现）、设置页的权限说明（`crates/lumilio-ui/src/pages/settings/plugins.rs`，新文案进目录）。测试：写 `xaero/minimap` 以外、`..`、绝对路径、符号链接、非 `mw$*.txt` 文件名都被拒绝，且没有任何文件被创建。
- [ ] T22：W7 的写入流程在 core（`world_map/write.rs`，新建）：取实例租约、核对长度/修改时间/sha256、备份、临时文件加 rename；`Effect` 不变，插件经 `write_file` 写。测试：游戏运行（租约被占）时没有任何写入；文件在读与写之间变了就拒绝；备份存在且与原文件一致；损坏文件不被覆盖。
- [ ] T23：W10 编辑契约：`MapObject::editable`、`OverlayProvider::apply`、`ObjectEdit`；UI 的编辑对话框（`crates/lumilio-ui/src/world_explorer/edit.rs`，新建），复用 `SettingKind::accepts` 校验。
- [ ] T24：路径点可改名称（不超过 32 个字符，拒绝换行）、缩写（不超过 2 个字符）、坐标、颜色（0–15；样本证明 16–19 有效后再开放，W17）、启用、传送朝向；在地图上点一下新建普通路径点；删除前确认，死亡点要单独确认；批量操作只作用于普通、启用、非临时的点。只改被编辑的那一行。
- [ ] T25：游戏运行时编辑入口禁用并说明原因；浏览和复制照常。

### P4 存档底图

- [ ] T26：`crates/lumilio-anvil/`（新建）：Region 头、四种压缩、外置 `.mcc`、1.18 前后的区块格式、群系调色板、`WORLD_SURFACE` / `MOTION_BLOCKING` 高度图、顶层方块。改编自 fastanvil `fastanvil/src/region.rs` 和 `fastanvil/src/java/`（MIT，注释署名）。测试用真实的小 Region 样本（自己用原版生成，不含第三方存档），覆盖 1.16、1.18+ 和最新版本；坏区块只让这一个区块为空。
- [ ] T27：宿主文件接口：`read_range`、`file_info`、`list_dir`（`access.rs`、`context.rs`）。测试：越界、链接、超出授权都被拒绝；大文件按区间读不受 32 MiB 上限影响，但单次区间有上限。
- [ ] T28：W13 方块颜色表：`cargo xtask block-colors <版本>`（`crates/lumilio-xtask/src/block_colors.rs`，新建）从客户端 jar 生成 `crates/lumilio-plugin-world-explorer/data/block-colors/<版本>.json` 和 `SOURCE.md`；先生成实现时最新的正式版和 1.21.4 两张。插件按世界版本选表，表外方块用「未知方块」色。测试：用一个合成的小 jar 跑工具；选表规则（不晚于世界版本的最新一张；比最早一张还旧时用最早一张）；表外方块计数。
- [ ] T29：存档底图 Provider：level 0 按顶层方块加高度明暗出瓦片（参考 fastanvil `render.rs` 的 `TopShadeRenderer`）；粗 LOD 由宿主合成。没生成过的区块画「无数据」。
- [ ] T30：增量更新：缓存记下 Region 文件的长度和修改时间；游戏退出后只重建变了的块。测试：改动一个 Region 文件，只有它和它的上层瓦片失效。
- [ ] T31：Overlay「存档位置」：实际出生点；单人存档 `level.dat` 里 `Player` 的位置和维度（若有）。

### P5 Xaero 世界地图底图

- [ ] T32：`lumilio-plugin-world-explorer/src/xaero/world_map.rs`（新建）：读区域 zip、解码区域流，改编自 XaeroTools `crates/xaero-core/src/codec/` 和 `render/mod.rs`（MIT，注释署名）。支持的版本范围以测试样本为准，其余报「不支持」。颜色用 W13 的表。
- [ ] T33：按可见区域加载，局部缺失画「无数据」；区域 zip 变了才重建。大存档（数千个区域文件）用 `list_dir` 分页，不一次列完。

### P6 自定义标记、路线、测距与下界换算

- [ ] T34：`launcher.db` 的标记与路线表（`world_map/store.rs`）：每条记录带世界、维度、坐标、名称、颜色和可选的关联（cubiomes 结构 key、Xaero 路径点的 `raw_id`）。关联的对象不见了，标记还在，并提示关联失效。
- [ ] T35：在地图上新建、编辑、删除标记（复用 W10 的对话框）；画路线（折线），显示总长；测距工具（两点间方块距离），不持久化。
- [ ] T36：下界 8:1 换算工具（W8）：选中的点或输入的坐标，给出另一维度的坐标，可以复制或跳转。测试：负坐标的取整与游戏一致（向下取整，待用实机核对）。

### P7 面向插件的扩展与 WASM（依赖 `wasm-plugin-registry.md` 的 T1、T4）

- [ ] T37：在 WASM 计划的 WIT 里加 `base-map-provider` 和 `overlay-provider`，类型与 `map.rs` 一一对应；瓦片字节和对象数量有上限；同样经过权限、超时和取消。示例 WASM 插件提供一个热力图 Overlay。
- [ ] T38：Litematica 插件实现 `OverlayProvider`，给出投影放置的位置和范围。投影放置信息存在哪个文件（Litematica 的配置目录），待核实；需要的 `ReadGameFiles` 范围随之扩大，设置页会显示。

### P8 新版本世界生成：1.21.5 → 26.3（延后 / 可选，W14）

所有功能阶段之后再做，不阻塞收尾。首个目标 26.3。1.21.5 到 26.2 之间的版本只在 T39 证明生成输入与 1.21.4 或 26.3 完全相同、且自己的金样通过时顺带支持（T43 列出结果），其余显示「这一版还不能查」。下面的已知差异来自 cubiomes issue #165；其余差异在 T39 之前一律待核实。依赖 P4 的 `lumilio-anvil`（T26）。

- [ ] T39：`cargo xtask worldgen-diff <旧版本> <新版本>`（`crates/lumilio-xtask/src/worldgen/`，新建）：按 Mojang 版本清单下载两个版本的服务端 jar 到被 git 忽略的 `target/worldgen/<版本>/`，跑数据生成器（W14 第 1 步），解出 `data/minecraft/worldgen/**` 与 `tags/worldgen/**`，输出差异摘要：主世界与下界的多噪声参数表、`noise_settings` 与密度函数、噪声参数、群系注册表、`structure_set`（salt、spacing、separation、频率、排除区）、结构的群系标签、新增结构。用得上的摘录连同 `SOURCE.md` 存进 `forks/cubiomes/data/<版本>/`。先对 1.21.4 → 1.21.5、…、26.2 → 26.3 两两跑一遍（具体的正式版列表以 Mojang 版本清单为准，待核实），结论写进 `forks/cubiomes/LUMILIO.md`。已知：26.2 新增硫磺洞穴（sulphur caves）群系（issue #165，在较低的 Y 才看得到，说明是洞穴群系、在多噪声参数表里）；它的参数、引入版本以及有没有别的变化，待核实。
- [ ] T40：`cargo xtask worldgen-golden <版本> <种子>…`：起一个本机、离线的原版服务端（固定 `level-seed`，托管 Java 由 ADR 0014 的运行时提供），对一组固定坐标 `forceload` 后用 `lumilio-anvil` 读区块的群系调色板（三个维度、几个高度），并经 RCON 在几个起点执行 `/locate structure`；结果写成 `crates/lumilio-cubiomes/tests/golden/<版本>/<种子>.json`（种子、维度、坐标、群系 id、结构种类与位置）。先对 1.21.4 跑，金样必须与未改动的上游 cubiomes 一致，证明工具本身没错。
- [ ] T41：`cargo xtask cubiomes-btree <参数表>`：从 `forks/cubiomes/data/<版本>/` 里的主世界参数表生成 `tables/btree<版本>.h`，树的构造是自己写的实现（按游戏的行为核对，不翻译游戏代码）。验收：用 1.21.4 的参数表生成的表与上游 `tables/btree21wd.h` 数值完全一致。
- [ ] T42：按 T39 的差异改 fork：`biomes.h` 新增 `MC_26_3` 等枚举项（命名跟随上游风格）与新群系 id（硫磺洞穴；若上游以后分配了 id，以上游为准，见 W14）；`tables/btree26_3.h`（若参数表变了）；`biomenoise.c` 按版本选树并处理噪声参数变化；`finders.c` 的 `getStructureConfig` 与可行群系列表（按 `structure_set` 和群系标签的差异）；下界参数表若有变化同样处理；`util.c` 的群系名称与配色（新群系先用相近群系的颜色，记进 `LUMILIO.md`）。每处改动一条 `LUMILIO.md` 记录。
- [ ] T43：若 T39 发现新结构，或已有结构的生成规则变化到现有 finder 表达不了，就为它写 finder（`finders.c`），并在 `lumilio-cubiomes` 开放；没有就在 `LUMILIO.md` 写明「26.3 无新结构」。中间版本逐个归类：生成输入与 1.21.4 相同（共用 1.21.4 的实现）、与 26.3 相同（共用 26.3 的实现）、都不同（不支持）。前两类采集各自的金样，通过后进入候选。
- [ ] T44：`lumilio-cubiomes/src/tests.rs` 用 T40 的金样逐点对比 26.3（至少三个种子 × 三个维度 × 几个高度，加上主要结构）以及 T43 的候选版本，全部一致；维护者按 W14 第 5 步实机核对 26.3 后，把 26.3 和通过的候选写进版本表（`lumilio-cubiomes/src/lib.rs`），并在 `LUMILIO.md` 记下核对日期。之后的新版本按同一流程（T39–T44）各加一次。

### 收尾（只依赖 P0–P7）

- [ ] T45：把冻结的决策压缩进下一个 ADR（编号到时再取，目前最大是 0037），写明对 D2 的修订（W15：瓦片和对象故障不进 `Failed`，同一个 Provider 连续 5 次只停用它）、对 D3（写权限）和 D5（`View::Map` 与编辑契约）的修订，以及 W13（入库的方块颜色表）、W14（自维护的 cubiomes fork、加版本的流程与入库规则）和 W16（地图只在游戏页）；删除本计划。P8 若还没做完，把 P8 的说明和 T39–T44 原样搬进新计划 `.agents/plans/cubiomes-new-versions.md`（`Status: proposed`，引用新 ADR 而不是本计划），再删本计划；若已做完，ADR 里加一行交付了哪些版本。

## 每阶段验收

- **P0**
  - 自动：`just test-pkg lumilio-cubiomes`（金样）、`just test-pkg lumilio-core world_map`、`just test-pkg lumilio-core plugins::tests`（瓦片故障不改变插件状态）、`just test-pkg lumilio-map-render`（GPU 像素）、`just test-pkg lumilio-ui world_explorer`（GPUI 交互：拖动后旧世代的瓦片不进入新视图；切换维度保留中心与缩放；关闭再开不吃旧结果）、`just test-pkg lumilio-docgen`（边界、署名、IA）、`just ia`、`just check`。
  - 自动：xtask 测试证明三种包都带 `ATTRIBUTIONS.md`；`ci.yml` 的 Windows 作业在 PR 上通过。
  - **维护者实机或目视**：Windows 上 `cubiomes` 用 MSVC 编译通过且探针结果与 macOS 相同；macOS 上拖动和缩放的手感；帧时间：记下 1800×1200 下拖动时每帧的毫秒数，写进实施记录，超过 8 毫秒就回到 W5 讨论；浅色和深色；720×480 和大窗口；没有显卡时的说明。
- **P1**
  - 自动：三个版本的结构坐标与金样一致；1.18+ 的估计标记存在。
  - **维护者实机**：用一个已知种子，对照 Chunkbase 或 Cubiomes Viewer 的同一处村庄和要塞。
- **P2**
  - 自动：样本往返逐字节一致；冒号、表情、没有 Y、未知行都能往返；损坏的文件显示错误。
  - 自动：取色器只给出 0–15（除非样本已证明 16–19）。
  - **维护者实机**：钉住版本（W17）的 Xaero 样本与游戏内列表一致；复制的分享串在游戏里能导入。
- **P3**
  - 自动：写权限测试证明插件碰不到 `xaero/minimap` 以外的路径；运行中的实例不会发出任何写入；冲突检测；备份；只有被改的那一行变化。
  - **维护者实机**：改一个点，重新进入世界后 Xaero 列表里看得到；游戏运行中编辑入口禁用。
- **P4**
  - 自动：Region 样本解析、坏区块隔离、增量失效；`block-colors` 工具与选表规则；每张入库的颜色表都有 `SOURCE.md`。
  - **维护者目视**：一个真实存档的地表看起来对（水、树、雪、下界）；退出游戏后新探索的区域出现；带模组方块的存档里「未知方块」的提示。
- **P5**
  - 自动：样本区域文件的解码与颜色；不支持的版本报错且不影响其他底图。
  - **维护者目视**：同一个世界的存档底图和 Xaero 底图切换时，中心、缩放和 Overlay 不变，地形大致对得上。
- **P6**
  - 自动：标记的增删改与持久化；关联失效的提示；8:1 换算。
  - **维护者实机**：路线和测距的交互。
- **P7**
  - 自动：WASM 示例插件的瓦片越权、超大、超时都被拒绝；Litematica Overlay 的范围与投影一致。
  - **维护者实机**：本地加载示例插件，热力图出现在图层面板；安全模式下消失。
- **P8（延后 / 可选）**
  - 自动：`just test-pkg lumilio-xtask worldgen`；T40 的工具在 1.21.4 上采到的金样与未改动的上游 cubiomes 一致；T41 由 1.21.4 参数表生成的表与 `btree21wd.h` 数值一致；`just test-pkg lumilio-cubiomes golden` 中 26.3 与各候选版本的金样逐点一致（三个维度、多个高度、主要结构）；`forks/cubiomes/data/` 下每个版本都有 `SOURCE.md`；仓库里没有 jar（`git ls-files '*.jar'` 为空）。
  - 自动：版本表之外的版本（例如 26.2，若它未被归入候选）仍返回 `Unsupported`，界面显示「这一版还不能查」。
  - **维护者实机**：在 26.3 的游戏里，用两个种子对照地图：F3 看几个采样点的群系（含硫磺洞穴等新群系，若 26.3 有），`/locate` 村庄和要塞与地图上的位置一致。核对之后才把 26.3 加进版本表。

## 风险

- 新版本延后：P8 做完之前，种子底图和结构 Overlay 只覆盖到 1.21.4；维护者和大多数玩家在用的 26.x 会显示「这一版还不能查」。功能阶段的实机验收要用 1.21.4 及更早的世界。
- 自维护 fork 的长期成本：Mojang 每个版本都可能改世界生成（新群系、参数表、噪声、结构配置），每次都要按 W14 跑一遍差异、改 C、采金样、实机核对。P8 是第一次，工作量最难估；之后每个大版本都要重复。
- 自己实现可能出错：多噪声树的构造、新群系的参数或结构规则理解错，地图就会画错而看起来很真。兜底是 W14 的金样（我们自己从真实世界测得）和维护者实机核对；金样只覆盖采样点，采样之外的错误仍可能漏掉，所以每个版本至少三个种子、三个维度、多个高度。
- 空窗期：一个新版本发布后，到我们完成五步之前，种子底图和结构 Overlay 在这个版本上显示「这一版还不能查」。玩家升级游戏后会先看到它。
- 上游 cubiomes 若恢复更新：要把上游的提交合进 fork，并逐个判断本地补丁是否被覆盖。两边对同一版本的实现不同时，以金样和实机为准（W14）；新群系的 id 若和我们自己分配的不同，要迁移种子瓦片缓存（缓存键含配色和版本，换 id 时清掉对应缓存）。
- 入库的派生数据会过时：方块颜色表和 `forks/cubiomes/data/` 的摘录都按版本生成，新版本加了方块或改了贴图就要重新跑工具。来源记录不全的表无法重新生成，所以 `SOURCE.md` 由工具写，不手写。
- cubiomes 里的 C 崩溃会直接带走整个启动器；按 W11 留在进程内，只能靠 Rust 侧的输入校验和测试降低概率。我们自己改的 C 代码增加了这个风险，新增的代码路径也要有越界和异常输入的测试。
- MSVC 编译和 Windows、Linux 上的 wgpu 离屏渲染都还没验证；CI 只覆盖 macOS。
- 离屏回读的帧时间在大窗口上可能不够平滑。
- Xaero 的格式没有公开规范，只有 XaeroTools 的逆向结论和社区记录，而且颜色下标的数量互相矛盾；新版本可能悄悄改格式。
- 存档格式每个大版本都可能变（26.x 的区块格式）；没有样本的版本只能报不支持。
- 2b2t 规模的 Xaero 存档有几万个区域文件，目录列举、缓存大小和内存都要有上限。
- wgpu 30 和 GPUI 的 wgpu 29 在同一进程里各开一个设备；schematic 预览已经这样跑，地图多一个设备，显存占用要观察。
- 冷编译时间再增加（C 库加一个 wgpu crate）。

## 开放问题

目前没有未决问题（2026-10-08 维护者采用了全部默认值）。

## 从旧计划迁移

`xaero-waypoints.md`：

| 旧条目 | 新位置 |
|---|---|
| Goal：按世界和维度看集合；改名称、缩写、坐标、颜色、启用、朝向；新建、删除；复制分享串；运行中只读 | P2 T17–T20、P3 T24–T25、W7 |
| Scope In：读写一个实例的路径点；集合切换；分享串；写权限只覆盖 `xaero/minimap` | T18、T20、T21、W7 |
| Scope Out：小地图渲染、游戏内传送、VoxelMap / JourneyMap 原生格式、运行中抢写、死亡点批量清空 | Scope Out（保留原文）；「小地图渲染」指游戏内渲染，Xaero 世界地图作为底图另由 P5 提供 |
| 调研：目录结构、维度目录、集合文件名 | 调研「Xaero 文件格式要点」，补充了 `_1.txt`、`.temp`、备份目录、world-map 树 |
| 调研：分享串格式、颜色 0–15 | 同上；补充了 20 色的说法与长度限制；下标规则见 W17 |
| 调研：磁盘字段多于分享串、`sets:` 与 `dim:` 头、以选定版本为准并在测试里钉住真实文件 | 同上；`dim:` 标为待核实；T17 钉样本 |
| 调研：JourneyMap 只做导入；本插件直接编辑 | Scope Out（JourneyMap 格式与导入） |
| 调研：「视图树适合列表和表单」 | 更正：视图树没有输入控件；改为 W10 声明式编辑 |
| 调研：窄写权限；Xaero 退出世界时写回、运行中只显示 | W7、T21、T25 |
| 调研：临时点和死亡点可见；默认删除和批量只作用于普通点；死亡点单独确认 | W7、T24 |
| T1 解析与写回、未知字段原样保留、三维度样本往返并注明版本 | T17（读与往返）、T22/T24（写） |
| T2 写权限、设置页一句话、读失败与损坏变成页面状态不进 `Failed` | T21、T18、W15 |
| T3 列出世界、维度、集合；行内编辑；新建；删除确认；复制分享串 | T18、T19、T20、T24（行内编辑改为地图上的对话框，见 W10） |
| T4 运行中禁用写入、保留浏览和复制 | T25、W7 |
| 验收：逐字节往返；损坏不覆盖；写权限测试；运行中不写；实机重新进入世界可见 | P2、P3 验收 |
| 开放问题：World Map 与小地图分叉时只支持小地图 | 调研里已回答（两棵树早已分开，路径点只在小地图），页面说明保留在 T18 |
| 开放问题：名称里的冒号和换行 | 调研里已回答：冒号写成 `§§`，换行拒绝（T24） |

`cubiomes-seed-map.md`：

| 旧条目 | 新位置 |
|---|---|
| Goal：三个维度的生物群系，叠加结构、史莱姆区块、出生点；平移缩放；开关图层；点结构复制坐标；后台计算，取消与切种子时丢弃过期图块 | P0、P1、W4 |
| Scope In：Java 版、cubiomes 支持的版本、当前实例的种子和版本、宿主画地图而插件只交图块和标记 | Scope、W2、W3、W9 |
| Scope Out：基岩版、Amidst / Chunkbase 的全部筛选条件、cubiomes 进 core、插件视图树直接画 GPU | Scope Out、W11、W9 |
| Scope Out：在地图上创建路径点 | **有意改变**：提案把「Waypoints 互动」列为包含项，P3 T24 支持在地图上新建 Xaero 路径点 |
| 调研：入口头文件、`Range` 的五档缩放、垂直 1:1 与 1:4 | 调研、W4 |
| 调研：Cubiomes Viewer 覆盖到 1.21、GPL-3.0；1.18+ 三种结构估计；1.18 前出生点误差 | 调研（参考项目表、种子与版本）、T14、T15 |
| 调研：GPLv3 §13 与 AGPL 组合；保留 Cubitect 的 GPL 声明 | **更正**：cubiomes 是 MIT，只需保留 MIT 声明（T11）；GPL 只涉及 cubiomes-viewer 与 Axolotl |
| 调研：宿主地图节点；请求带种子、版本、维度、缩放、范围；请求 id 丢弃迟到结果 | W4、W9（`View::Map`）、T2–T4 |
| 调研：5 秒超时与失败停用不适合；图块可取消、失败只重试这一块 | W9、W15、T3 |
| 调研：种子来自 `level.dat`；没有世界时粘贴种子并写明版本 | 调研（种子与版本）、T4、T9 |
| T1 构建脚本编译 cubiomes；群系与结构两个包装；不支持的版本明确报错；用已知种子对照 | T0、T1、T13 |
| T2 地图节点与取消令牌；新视口取消旧任务；超时或崩溃只影响这一次 | T2、T3、T4 |
| T3 读到种子与数据版本后显示地图；图层列表；点标记复制 `x z` | T6、T7、T14–T16 |
| T4 1.18+ 估计提示；没有 `level.dat` 时手动种子 | T14、T9 |
| 验收：三个版本的结构坐标与「cubiomes 命令行」一致 | P1 验收；cubiomes 没有现成的命令行，改为仓库内 C 探针生成的金样 |
| 验收：快速拖动时旧图块不画进新种子或新维度 | P0 验收 |
| 验收：插件崩溃测试，宿主还在，地图显示失败状态 | T3、P0 验收 |
| 验收：构建产物的归属声明包含 cubiomes 的「GPL 通知」 | T11、P0 验收（改为 MIT 声明，并真正放进包里） |
| 验收：实机对照 Chunkbase 或 Cubiomes Viewer | P1 验收 |
| 开放问题：1.21 之后等上游，不降级 | **有意改变**：不等上游，由 LumilioCL 在 `forks/cubiomes` 自己补（W14），放在延后阶段 P8，首个目标 26.3；「不降级」保留，版本表外的版本显示「这一版还不能查」 |
| 开放问题：第一轮用 cubiomes 自带的 Amidst 风格色表并注明，不做主题化配色 | T6、T11、参考项目表 |

没有有意舍弃的条目。改变的有四处，上表都已标出：在地图上新建路径点从 Out 变成 In；cubiomes 的许可证从 GPL 更正为 MIT；「命令行输出」改为 C 探针生成的金样；新版本不再等上游，而是自己在 fork 里补（维护者 2026-10-08 的决定，放在延后阶段 P8）。

## References

- ADR：0006、0011、0014（托管 Java，P8 金样工具起服务端用）、0019、0021、0022、0025、0026、0027、0028、0029、0031（D1–D7）、0034（读过 26.3 不混淆的客户端）、0036
- 计划：`.agents/plans/wasm-plugin-registry.md`、`.agents/plans/i18n-english.md`（T2c）
- 本仓库：`crates/lumilio-plugin-api/src/{lib.rs,view.rs,analysis.rs}`、`crates/lumilio-core/src/plugins/{mod.rs,access.rs,context.rs,tabs.rs,model.rs}`、`crates/lumilio-core/src/{worlds.rs,layout.rs,instance.rs,model_assets.rs}`、`crates/lumilio-core/src/service/{launch.rs,models.rs,worlds.rs}`、`crates/lumilio-nbt/src/lib.rs`、`crates/lumilio-schematic-render/src/{scene.rs,view.rs}`、`crates/lumilio-ui/src/model_view/{mod.rs,worker.rs,modal.rs}`、`crates/lumilio-ui/src/instance_detail/plugin_tabs.rs`、`crates/lumilio-ui/src/pages/settings/plugins.rs`、`crates/lumilio-app/src/backend.rs`、`crates/lumilio-docgen/src/lib.rs`、`crates/lumilio-docgen/tests/{plugin_boundaries.rs,attribution.rs}`、`crates/lumilio-pointer/Cargo.toml`、`crates/lumilio-xtask/src/{macos,windows,linux,release}.rs`、`forks/README.md`、`ATTRIBUTIONS.md`、`justfile`、`.github/workflows/{ci.yml,release.yml}`
- cubiomes：<https://github.com/Cubitect/cubiomes>（`e61f905`；`generator.h`、`finders.h`、`biomes.h`、`rng.h`、`util.c`、`biomenoise.c`、`tables/btree21wd.h`、`docs/nptree_c.py`、`LICENSE`）；issue #161、#165
- cubiomes-viewer：<https://github.com/Cubitect/cubiomes-viewer>（`3acc863`；README「Known issues」「Legal information」）
- Axolotl：<https://github.com/Mystic-Stars/Axolotl>（`b957550`；`apps/app/src/seed_map/{cubiomes_bridge.c,cubiomes_bridge.h,mod.rs,ores.rs,rng.rs}`、`apps/app/src/api/seed_map.rs`、`apps/app/build.rs`、`apps/app-frontend/src/pages/LabSeedMap.vue`、`apps/app-frontend/src/lab/seed-map/features.ts`、`apps/app/COPYING.md`）
- fastnbt / fastanvil：<https://github.com/owengage/fastnbt>（`986c859`；`fastanvil/src/{region.rs,render.rs,java/chunk.rs,java/section_data.rs}`、`palette.tar.gz`）
- XaeroTools：<https://github.com/dekrom/xaerotools>（`7bc650b`；`crates/xaero-core/src/{waypoints.rs,naming.rs,dimconfig.rs,codec/,render/}`、`crates/xaerotools-server/src/pyramid.rs`、`assets/colortable.bin`）
- Amidst：<https://github.com/toolbox4minecraft/amidst>（`LICENSE.txt`，GPLv3）
- Xaero 分享串记录：<https://gist.github.com/macimas/937a392be075b1bce7a2ae69ea933ef5>
- 颜色下标：<https://github.com/rfresh2/XaeroPlus/issues/301>
- JourneyMap 的目录匹配：<https://teamjm.github.io/journeymap-docs/6.0.x/client/waypoints>
- 仓库根目录 `LICENSE`（AGPL-3.0-only）

## 实施记录

- 2026-10-08：缩小再放大会卡死 App，原因有三，都已修：① `frame()` 每来一块瓦片就在界面线程克隆全部可见瓦片的 RGBA（几百块 × 256 KiB，二次方增长），现在瓦片像素到达时只准备一次，`Tile.rgba` 是 `Arc<[u8]>`，占位图案共用一张纹理；② LOD 在 1–4 方块/像素之间仍用 level 0，缩小时同时生成数百块 1 方块/像素的瓦片，现在按「纹理一格不超过两个屏幕像素」选层（`Camera::level`）；③ 每块瓦片在插件里逐行调用 cubiomes 256 次，每次重新 `applySeed`，现在整块一次初始化、每 16 行一个条带并在条带间轮询取消（`biomes_until`）。另：界面一次最多在途 12 块（`IN_FLIGHT`），完成一块再补一块；瓦片缓存 512 块（`TILE_CAP`），其他层级的已缓存瓦片在新层级就绪前垫底，缩放不再露出空洞。参考 Axolotl（`apps/app-frontend/src/pages/LabSeedMap.vue` 的 `enqueueTile` 并发上限与按世代丢弃）的做法，没有复制其代码。
- 2026-10-08：布局按桌面反馈调整：地图窗格补上 `BOTTOM_SAFE_AREA`，不再被导航栏盖住；地图下方不再有文字或控件，维度/底图在地图左上、坐标跳转在右上（X、Z 两个输入框加「前往」键）、光标坐标和状态在左下、图层在右下；种子行仍在地图上方。新布局目视验收仍待维护者。

- 2026-10-08：按 Edwin 的桌面反馈修正 P0 控件：种子/坐标改行内 Input，版本/存档改可搜索 Select，底图/维度用 Segments，图层移到右下角浮层并用开关与粗缩放提示，失败块统一重试，地图填满剩余高度；补 GPUI 交互守卫和 AGENTS 控件选择约定，新布局目视验收仍待维护者。

- 2026-10-08：维护者在 PR #6 上决定：cubiomes 作为 LumilioCL 自维护的 fork（`forks/cubiomes`），自行补 1.21.5–26.x 的世界生成，首个交付 26.3（W14、P2）；瓦片和对象故障不进 `Failed`，同一个数据源连续 5 次出错只停用它（W15，修订 ADR 0031 D2）；地图只放在游戏页的插件标签里（W16）。
- 2026-10-08：维护者在 PR #6 上决定：允许入库从 Mojang 资料派生的生成数据（记录生成命令与来源，W13、W14）；新版本世界生成支持延后到功能完成之后（当时的 P2 改为延后阶段 P8，不阻塞收尾 T45）。上一条里的「P2」指当时的编号。
- 2026-10-08：维护者在 PR #6 上采用了剩下 11 个开放问题的全部默认值，已并入冻结决策：帧时间测量与 8 毫秒阈值 → W5；LOD 每级 4 倍 → W4；标记存 `launcher.db`（schema 4）与缓存上限 1 GiB → W6；cubiomes 留在进程内 → W11；默认启用与不升 `API_VERSION` → W9；Xaero 钉最新 26.x 版与颜色下标先开放 0–15 → W17；Windows 编译作业与发布包带 `ATTRIBUTIONS.md` → W18。
