# 宣传网站与下载入口 launcher.lumilio.org

- Status: in_progress

## Goal

`launcher.lumilio.org` 是 LumilioCL 的主站：一个静态宣传页，加上同一个 Cloudflare Worker 里的
GitHub Release 反向代理。页面用启动器自己的设计语言（铝面板、按键、LED、显示屏、步进的像素
世界），能直接下载对应平台的安装包；中国大陆的下载经 Worker 转发。

## Scope

- In：`web/` 下的 Astro + React 站点（i18n 结构，先只写中文）；首屏「logo 面板」探索视窗；视差与横向滚动
  （版块之间不做滚动吸附，维护者试过，手感别扭）；下载区；Worker 的 `/releases/*` 镜像；部署 workflow。
- Out：截图、GIF 和视频素材（维护者提供，页面留占位与注释）；英文文案；`/plugins/*`
  （`wasm-plugin-registry.md` 的 T6 (b)(c)）；启动器内的自动更新（`release-0-1-0.md`，参考 Zed）。

## 决定

- 一个 Worker 同时托管静态资源和反代，不用 Pages：主站和反代同域名，一次部署。
- 代码在本仓库 `web/`。字体、图标、截图由 `web/scripts/sync-assets.mjs` 从
  `crates/lumilio-ui/assets/fonts`、`assets/icons`、`assets/screenshots` 复制进 `public/`，
  不在 `web/` 里存第二份。
- 反代照 gh-proxy：流式转发、不做边缘缓存，只放行 `EdwinZhanCN/LumilioCL` 与跳转白名单。
- logo 九宫格的含义：黑格是已经有的功能，灰格是路线图（0.1.1 / 0.2.0 / 0.3.0），橙色圆是下载。

## References

- `docs/design-language.md`（§1 两种语域、§2 动效 token、§12 色板与控件、§13 字体）
- `assets/icons/src/tile/*.svg`、`crates/lumilio-ui/src/hero/scenes/*.rs`（像素场景的配色）
- gh-proxy（hunshcn/gh-proxy，MIT）的 Worker 实现

## Tasks

- [x] S1：骨架。`web/` Astro 7 + React，i18n 结构，字体与色板 token，亮暗跟随系统并可切换。
- [x] S2：首屏 logo 面板：格子悬停、黑格展开成探索窗口、灰格显示路线图、橙色圆的启动动画。
- [x] S3：其余部分：实测（与 Modrinth App 的内存、进程数、安装大小）、三类用户（横向滚动）、
  原生技术层（分层展开的视差）、路线图、下载区、页脚。实测数据在 `web/src/data/benchmark.ts`，
  文件头写着重测方法；发新版本后要重测并更新。
- [x] S4：Worker 反代与测试；静态资源与反代合一的 `wrangler.jsonc`。
- [ ] S5：`ci.yml` 排除 `web/**`、`web.yml` 检查与部署（已写）；仓库加 `CLOUDFLARE_API_TOKEN`、
  `CLOUDFLARE_ACCOUNT_ID` 两个 secret，首次 `wrangler deploy` 绑定 `launcher.lumilio.org`（维护者做）。
- [ ] S6：维护者补截图与 GIF，按 `TODO(asset)` 注释替换占位；og:image。

## Validation

- `just web`：astro check 0 错误、Worker 测试 9 个通过、构建通过。
- 已用无头 Chromium 截图看过：1440×900 亮 / 暗、390×844、减少动画、探索窗口、启动动画、
  下载区（用 `LUMILIO_RELEASE_FIXTURE` 假数据看了有安装包时的样子）。
- 反代：`wrangler dev` 本地验证了路由分流（静态 / `/releases/*`）、405、非法 tag；一次性测试
  经真实 GitHub 跳转流式取回别的仓库的资源（200 与 Range 206）。本仓库还没有 Release，
  所以没有拿 LumilioCL 自己的安装包实测过。
- 还要维护者看：真实浏览器里的滚动手感（吸附、横向段落、视差）、Safari 与 Firefox、
  首屏格子的键盘操作。

## 记录

- `astro check` 不支持 TypeScript 7，`web/` 固定在 TypeScript 6。
- Astro 7 的 `astro dev` 会自己转入后台，用 `astro dev stop` 停。
- 用 `transform` 动画的祖先会成为 `position: fixed` 的包含块：探索窗口因此 portal 到 `body`。

## Open questions

- 截图与 GIF 的最终清单，见页面里的 `TODO(asset)` 注释。
- 自动更新（参考 Zed）的清单格式定下后，Worker 是否要加 `/update/*`；现有
  `/releases/latest/download/<file>` 已能代理固定文件名的清单。
