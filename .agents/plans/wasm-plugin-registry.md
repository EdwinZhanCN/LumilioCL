# WASM 插件与可审核的社区目录

- Status: proposed

## Goal

社区插件以 WASM 运行，能力只能通过宿主的 WIT 接口获得。随应用发布的四个插件仍是核心插件。社区插件默认不运行；启动器提供受限模式和安全模式。社区目录只列出经过扫描、版本被哈希钉住的插件，安装源默认是这个目录，而不是任意网址。

## Scope

- In：Wasmtime 宿主、与现有扩展点对应的 WIT、核心 / 社区区分、受限模式、安全模式、目录客户端、安装与卸载、权限展示、开发期从本地目录加载。
- Out：把现有四个核心插件改写成 WASM、插件内自由画 UI、插件市场付款、自动更新启动器、要求社区插件使用 AGPL。

## 调研

Zed 的扩展是一个带 `extension.toml` 的 Git 仓库。过程代码用 Rust 编译到 `wasm32-wasip2`，由 Wasmtime 加载；宿主按 WIT 版本实例化扩展。扩展不能直接碰文件系统和网络，HTTP 与进程由宿主能力提供。本地开发用 Install Dev Extension，不必先发布（<https://zed.dev/docs/extensions/developing-extensions>）。Zed 在 2026 年修过一次沙箱逃逸，问题不在 Wasmtime，而在扩展压缩包解压时路径没有约束（<https://www.techtimes.com/articles/325481/20260825/zed-extension-sandbox-escape-patched-archive-layer-bypassed-wasm-isolation.htm>）。所以「用了 WASM」不等于安装包安全，解压必须拒绝跳出目标目录的路径。

Obsidian 把插件分成核心与社区。核心插件随应用提供。社区插件默认被 Restricted Mode 挡住，已安装的文件还在，但不会执行；用户要显式打开社区插件（<https://obsidian.md/help/plugin-security>）。Obsidian 写明自己无法把社区插件限制在权限内，因为它们是同一进程里的 JavaScript。我们不照搬这个弱点。WASM 与 WIT 能做到 Zed 那种能力边界，权限仍按 ADR 0031 的主机、目录和启动事件检查。

Obsidian 的目录策略是：每个版本自动扫描漏洞、代码质量和恶意软件，目录页显示 safety scorecard；热门、推荐和被标记的插件再人工看（<https://obsidian.md/help/plugin-security>）。开发者政策要求插件尊重离线与私人使用（<https://docs.obsidian.md/community-directory/developer-policies>）。本仓库没有现成的审核团队。第一轮的「可审核」是：目录条目含 id、版本、下载地址、sha256、声明的权限和审核状态；安装前校验哈希；自动扫描拒绝未声明的网络主机和压缩包路径穿越；第一版必须有人工标记才进入默认可安装列表。没有审核标记的条目可以浏览，不能一键安装。

安全模式覆盖「插件让程序起不来」：命令行 `--safe`，以及连续启动失败后下一次自动进入。安全模式只加载核心插件，社区插件保持安装但不执行。受限模式是日常开关，只禁止社区插件，核心插件仍按各自的启用状态运行。两者都要在设置里看得见原因。

现有四个插件继续以原生 crate 注册。WASM 插件使用同一套扩展点：`Analyzer`、`InstanceTab`、`ContentSource`、`LaunchObserver`，以及声明式设置。不给 WASM 插件 `Native(DiscordIpc)`。视图仍由宿主渲染。

目录本身是一份签名的索引，插件文件留在开发者自己的 GitHub Release，见下文「冻结的决策」。客户端不执行索引里的脚本。

## 冻结的决策

- **R1 目录模型：Obsidian 模式加哈希钉住。**（维护者 2026-10-09 决定）
  - 插件文件 `plugin.wasm`（component）和 `manifest.json` 放在开发者自己仓库的 GitHub Release 里，与 Obsidian 一样。不接受 prerelease 和 draft。
  - 目录仓库是 `EdwinZhanCN/lumilio-plugins`，每个插件一个 `plugins/<id>.toml`：`id`、`name`、`description`、`repo`、`owner`，以及若干 `[[versions]]`。每个版本记录 `version`、`tag`、`wasm_url`、`manifest_url`、`wasm_sha256`、`manifest_sha256`、`wasm_size`、`wit_api`、`min_app` 和 `permissions`。旧版本条目保留，客户端可以从中挑兼容的版本。撤回写在 `revocations.toml`：`id`、`versions`（`*` 或列表）、`reason`、`action`（`disable`，或者 `disable_and_suggest_uninstall`）。
  - 一个 PR 只新增或更新一个插件；改 `revocations.toml` 只限维护者。**合并 PR 就是人工审核。**
  - 开发者删除 Release 后，这个版本就不能再装了；但文件不可能被偷偷换掉，因为 sha256 对不上。
  - 从源码构建是以后的升级（T8），v1 不做。
- **R2 PR CI 不用任何密钥**，只用默认的 `GITHUB_TOKEN` 读公开数据。依次检查：
  1. 范围：一个 PR 只改一个 `plugins/<id>.toml`。
  2. id 规则：`^[a-z0-9][a-z0-9-]{2,40}$`，不能含 `lumilio`，不能与核心插件或已有 id 重名，先到先得。
  3. 归属：新条目要求 PR 作者是仓库 owner，或在该仓库有 admin 权限；已有条目要求 PR 作者是登记的 `owner`。不符就转人工。
  4. 资源：URL 必须在 `repo` 的 `releases/download/<tag>/` 下；下载后核对 sha256 和大小；Release 不能是 prerelease 或 draft。
  5. 大小：wasm 超过 10 MiB 直接失败；超过 2 MiB，或比上一版大一倍以上，只警告。
  6. 内容：manifest 的 `id` 和 `version` 与条目一致，版本号递增。wasm 是合法的 component，world 是 `lumilio:plugin@<wit_api>`，且宿主支持这个版本。imports 只能是宿主接口和白名单内的 WASI，不能有 `wasi:sockets` 或文件系统预打开。声明的权限与条目一致，格式符合 ADR 0031 D4。
  7. 权限对比：与上一版相比新增的主机、目录和事件，在 PR 评论报告里醒目标出。
- **R3 索引和签名。**
  - 合并后 CI 生成 `index.json`，`serial` 只增不减。Edwin 在本机用 `just sign-index` 做 minisign（ed25519）签名，再提交 `index.json.minisig`。私钥不进 CI。
  - 索引经 GitHub Pages 发布。
  - CI 每天读一次各开发者 Release 的 `download_count`，生成 `stats.json`。这个文件不签名，客户端也不做遥测。
- **R4 统一下载入口。**（维护者 2026-10-08 决定）
  - `launcher.lumilio.org` 是一个 Cloudflare Worker，参照 gh-proxy 做流式反向代理，带边缘缓存。唯一的源站是 GitHub Releases（索引的源站是目录仓库的 Pages）。不用 R2，也不自建服务器。
  - Worker 只接受 GET 和 HEAD，自己跟随 GitHub 的 302，最多 5 跳。跳转目标只允许 `github.com`、`release-assets.githubusercontent.com`、`objects.githubusercontent.com`。支持 HTTP Range，用 ReadableStream 直接透传响应体。
  - 缓存：带版本号的文件 `s-maxage` 设得很长；manifest 和索引只缓存约 60 秒。安装包小于 100 MB。
  - 代理三类路径：
    - (a) LumilioCL 自己的安装包和自动更新，来自 `EdwinZhanCN/LumilioCL` 的 Release，与 `release-0-1-0.md` 相关。
    - (b) `/plugins/index.json`（和 `.minisig`），短 TTL。
    - (c) `/plugins/<id>/<version>/<file>`。Worker 根据签名索引查出这个版本对应的 repo 和 tag，只放行已登记的 repo 和版本，不是开放的 GitHub 代理。
  - Worker 不校验 sha256，因为那样就没法流式转发。它只是不受信任的传输层，签名、serial 和 sha256 全由启动器校验。Worker 出故障时，客户端退回 GitHub 官方 URL。
- **R5 启动器。**
  - 公钥编进二进制，留两个位置以便轮换。索引签名不对、`serial` 比本地小，一律拒绝。拉取失败就沿用上次的缓存；连缓存都没有，社区列表就是空的，应用照常启动。
  - 索引每天最多检查一次，用 ETag 做条件请求；出错或遇到 429 时退避，429 后至少等一小时。客户端不调用 `api.github.com`。
  - 先经 Worker 下载，失败再走索引里的 GitHub URL。sha256 不一致就拒绝安装，并说明：开发者改动了这个版本的文件，与审核时不一致。
  - 已安装的文件按 `plugins/community/<id>/<sha256>.wasm` 存一份本地副本，运行时只用这份，所以 Release 被删也不影响已安装的插件。
  - 安装后默认停用；新版本多要了权限，必须重新确认。命中撤回列表的插件会被停用并说明原因，但不删除文件。
  - 有更新只提示，不自动更新。

Worker 参考草图（不是最终代码）：

```js
const LAUNCHER_REPO = "EdwinZhanCN/LumilioCL";
const INDEX_ORIGIN = "https://edwinzhancn.github.io/lumilio-plugins";
const REDIRECT_HOSTS = new Set(["github.com", "release-assets.githubusercontent.com", "objects.githubusercontent.com"]);

export default {
  async fetch(req, env, ctx) {
    if (req.method !== "GET" && req.method !== "HEAD") return new Response(null, { status: 405 });
    const url = new URL(req.url);
    let upstream, ttl;
    if (url.pathname === "/plugins/index.json" || url.pathname === "/plugins/index.json.minisig") {
      upstream = INDEX_ORIGIN + url.pathname.replace("/plugins", ""); ttl = 60;
    } else if (url.pathname.startsWith("/plugins/")) {
      const [, , id, version, file] = url.pathname.split("/");
      const index = await (await fetch(INDEX_ORIGIN + "/index.json", { cf: { cacheTtl: 60 } })).json();
      const v = index.plugins.find(p => p.id === id)?.versions.find(v => v.version === version);
      const asset = v && { "plugin.wasm": v.wasm_url, "manifest.json": v.manifest_url }[file];
      if (!asset) return new Response("not registered", { status: 404 });
      upstream = asset; ttl = 31536000;
    } else if (url.pathname.startsWith("/releases/download/")) {   // 启动器安装包与自动更新
      upstream = `https://github.com/${LAUNCHER_REPO}${url.pathname}`; ttl = 31536000;
    } else return new Response(null, { status: 404 });

    const headers = new Headers();
    if (req.headers.has("range")) headers.set("range", req.headers.get("range"));
    let res, target = upstream;
    for (let hop = 0; hop <= 5; hop++) {
      res = await fetch(target, { method: req.method, headers, redirect: "manual", cf: { cacheTtl: ttl, cacheEverything: true } });
      if (res.status < 300 || res.status >= 400) break;
      target = new URL(res.headers.get("location"), target).toString();
      if (!REDIRECT_HOSTS.has(new URL(target).hostname) || hop === 5) return new Response("bad redirect", { status: 502 });
    }
    const out = new Response(res.body, res);   // ReadableStream 透传，支持 206
    out.headers.set("cache-control", `public, s-maxage=${ttl}`);
    return out;
  },
};
```

## References

- ADR 0031，特别是 D1–D7 和「WASM、社区插件尚未开始」
- Zed 扩展开发：<https://zed.dev/docs/extensions/developing-extensions>
- Zed 扩展生命周期：<https://zed.dev/blog/zed-decoded-extensions>
- Obsidian Restricted Mode：<https://obsidian.md/help/plugin-security>
- Obsidian 开发者政策：<https://docs.obsidian.md/community-directory/developer-policies>
- Obsidian 社区插件拉取方式：<https://github.com/obsidianmd/obsidian-releases>
- Zed 发布流程：<https://zed.dev/docs/extensions/publishing/publishing-guide>
- GitHub REST 限额：<https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api>
- GitHub Pages 限额：<https://docs.github.com/en/pages/getting-started-with-github-pages/github-pages-limits>
- Cloudflare Workers 限额：<https://developers.cloudflare.com/workers/platform/limits/>

## Tasks

- [ ] T1：引入 Wasmtime 与 WIT。一个示例插件实现只读分析器，编译为 `wasm32-wasip2`。宿主按能力调用，插件访问未授权路径或主机时失败。解压测试包含 `../` 路径，必须拒绝且不落盘。同时定下发布格式（`plugin.wasm` 加 `manifest.json`），以及 `wit_api` 的版本规则：world 名是 `lumilio:plugin@<主.次>`，宿主声明自己支持哪些版本。
- [ ] T2：插件来源分为核心与社区。设置页分两组。受限模式默认开启。`--safe` 与连续启动失败进入安全模式，只留核心插件。退出安全模式要人确认。
- [ ] T3：目录客户端（R5）。
  - 索引：验签名和 serial，按 ETag 缓存，失败退避。
  - 下载：先走 Worker，失败退回 GitHub。
  - 校验与存放：核对 sha256，本地留副本。
  - 权限：展示权限，新增权限要重新确认。
  - 版本与更新：选兼容版本，处理撤回，有更新只提示。
  - 已审核版本可以安装、启用、停用、卸载。本地开发目录要单独确认，不进默认列表。
- [ ] T4：示例插件的崩溃、超时和取消不拖垮 UI，行为与 ADR 0031 的原生隔离一致。WASM 调用同样不在 UI 线程上阻塞。
- [ ] T5：建 `EdwinZhanCN/lumilio-plugins`（R1–R3）。包括 toml 格式、`revocations.toml`、PR CI 和报告评论、索引生成、`just sign-index`、Pages 发布，以及每天生成 `stats.json` 的任务。
- [ ] T6：Cloudflare Worker `launcher.lumilio.org`（R4）。三类路径（a/b/c）、跳转白名单、Range、缓存头、只放行已登记的版本；`wrangler.toml` 放在 `lumilio-plugins` 里。安装包下载这部分与 `release-0-1-0.md` 有关；自动更新不在 0.1.0 的范围里，等以后的计划再接。
- [ ] T7：端到端走一遍：示例插件发 Release → 提 PR → 合并、签名 → 启动器经 Worker 安装并启用。之后再验证：篡改 Release 文件后拒绝安装；撤回后插件被停用；删掉 Release 后已安装的插件仍能运行；Worker 停掉时退回 GitHub URL 仍能安装。
- [ ] T8（以后）：从源码构建。PR 里写 commit，CI 用固定工具链编 `wasm32-wasip2`，比对开发者上传的 sha256，或者把产物发布到 `lumilio-plugins` 自己的 Release 上。仍然不用 R2。

## Validation

- 示例插件在授权范围内返回结果；越权、损坏的 wasm、错误的哈希、压缩包路径穿越都被拒绝。
- 受限模式和安全模式下，已安装的社区插件不会被调用。
- 核心插件在安全模式下仍可开关。
- 索引签名不符、serial 回退、sha256 不符时都不安装。
- Worker 拒绝未登记的 repo 和版本，以及白名单之外的跳转。
- 实机：从本地目录加载示例插件，再打开安全模式确认它消失，核心插件还在。

## Open questions

- 人工审核没有人值守。计划只保证没有合并进目录的包不能从默认目录安装。
- 托管位置和签名密钥已经由 R3、R4 定下。

## 实施记录

- 2026-10-09：维护者定下目录模型。插件放在开发者的 GitHub Release，目录仓库按 sha256 钉住版本，PR 合并即审核，索引由维护者在本机用 minisign 签名，不用 R2 和 D1（R1–R3）。统一下载入口 `launcher.lumilio.org`（Cloudflare Worker 反代 GitHub Releases）沿用 2026-10-08 的决定（R4）。任务改为 T1–T8。
