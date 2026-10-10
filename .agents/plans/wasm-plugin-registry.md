# WASM 插件与可审核的社区目录

- Status: proposed

## Goal

社区插件以 WASM 运行，能力只能通过宿主的 WIT 接口获得。随应用发布的五个插件仍是核心插件。社区插件默认不运行；启动器提供受限模式和安全模式。社区目录只列出经过扫描、版本被哈希钉住的插件，安装源默认是这个目录，而不是任意网址。

## Scope

- In：Wasmtime 宿主、与现有扩展点对应的 WIT、核心 / 社区区分、受限模式、安全模式、目录客户端、安装与卸载、权限展示、开发期从本地目录加载。
- Out：把现有四个核心插件改写成 WASM、插件内自由画 UI、插件市场付款、自动更新启动器、要求社区插件使用 AGPL。

## 调研

Zed 的扩展是一个带 `extension.toml` 的 Git 仓库。过程代码用 Rust 编译到 `wasm32-wasip2`，由 Wasmtime 加载；宿主按 WIT 版本实例化扩展。扩展不能直接碰文件系统和网络，HTTP 与进程由宿主能力提供。本地开发用 Install Dev Extension，不必先发布（<https://zed.dev/docs/extensions/developing-extensions>）。Zed 在 2026 年修过一次沙箱逃逸，问题不在 Wasmtime，而在扩展压缩包解压时路径没有约束（<https://www.techtimes.com/articles/325481/20260825/zed-extension-sandbox-escape-patched-archive-layer-bypassed-wasm-isolation.htm>）。所以「用了 WASM」不等于安装包安全，解压必须拒绝跳出目标目录的路径。

Obsidian 把插件分成核心与社区。核心插件随应用提供。社区插件默认被 Restricted Mode 挡住，已安装的文件还在，但不会执行；用户要显式打开社区插件（<https://obsidian.md/help/plugin-security>）。Obsidian 写明自己无法把社区插件限制在权限内，因为它们是同一进程里的 JavaScript。我们不照搬这个弱点。WASM 与 WIT 能做到 Zed 那种能力边界，权限仍按 ADR 0031 的主机、目录和启动事件检查。

Obsidian 的目录策略是：每个版本自动扫描漏洞、代码质量和恶意软件，目录页显示 safety scorecard；热门、推荐和被标记的插件再人工看（<https://obsidian.md/help/plugin-security>）。开发者政策要求插件尊重离线与私人使用（<https://docs.obsidian.md/community-directory/developer-policies>）。本仓库没有现成的审核团队。第一轮的「可审核」是：目录条目含 id、版本、下载地址、sha256、声明的权限和审核状态；安装前校验哈希；自动扫描拒绝未声明的网络主机和压缩包路径穿越；第一版必须有人工标记才进入默认可安装列表。没有审核标记的条目可以浏览，不能一键安装。

安全模式覆盖「插件让程序起不来」：命令行 `--safe`，以及连续启动失败后下一次自动进入。安全模式只加载核心插件，社区插件保持安装但不执行。受限模式是日常开关，只禁止社区插件，核心插件仍按各自的启用状态运行。两者都要在设置里看得见原因。

现有五个插件继续以原生 crate 注册。WASM 插件计划映射现有扩展点：`Analyzer`、`InstanceTab`、`ContentSource`、`LaunchObserver`、`BaseMapProvider`、`OverlayProvider`，以及声明式设置。地图扩展的 WIT 对接需要随实现一起验证。不给 WASM 插件 `Native(DiscordIpc)`。视图仍由宿主渲染。

目录本身是一份签名的索引加上插件包。第一轮索引可以是仓库里的一个 JSON，由维护者合并审核结果后发布。客户端不执行索引里的脚本。

## References

- ADR 0031，特别是 D1–D7 和「WASM、社区插件尚未开始」
- Zed 扩展开发：<https://zed.dev/docs/extensions/developing-extensions>
- Zed 扩展生命周期：<https://zed.dev/blog/zed-decoded-extensions>
- Obsidian Restricted Mode：<https://obsidian.md/help/plugin-security>
- Obsidian 开发者政策：<https://docs.obsidian.md/community-directory/developer-policies>

## Tasks

- [ ] T1：引入 Wasmtime 与 WIT。一个示例插件实现只读分析器，编译为 `wasm32-wasip2`。宿主按能力调用，插件访问未授权路径或主机时失败。解压测试包含 `../` 路径，必须拒绝且不落盘。
- [ ] T2：插件来源分为核心与社区。设置页分两组。受限模式默认开启。`--safe` 与连续启动失败进入安全模式，只留核心插件。退出安全模式要人确认。
- [ ] T3：目录索引、哈希校验、权限与扫描结果的展示。已审核版本可以安装、启用、停用、卸载。未审核版本和本地开发目录要单独的确认，不进入默认列表。
- [ ] T4：示例插件的崩溃、超时和取消不拖垮 UI，行为与 ADR 0031 的原生隔离一致。WASM 调用同样不在 UI 线程上阻塞。

## Validation

- 示例插件在授权范围内返回结果；越权、损坏的 wasm、错误的哈希、压缩包路径穿越都被拒绝。
- 受限模式和安全模式下，已安装的社区插件不会被调用。
- 核心插件在安全模式下仍可开关。
- 索引签名或哈希不符时不安装。
- 实机：从本地目录加载示例插件，再打开安全模式确认它消失，核心插件还在。

## Open questions

- 公开索引的托管位置和签名密钥由维护者决定。代码先接受一个 HTTPS 上的索引地址，默认指向尚未建立的官方索引；索引不存在时社区列表为空，而不是失败打开应用。
- 人工审核没有人值守。计划只保证未审核的包不能从默认目录安装。
