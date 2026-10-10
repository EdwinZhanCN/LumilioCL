# 自动更新（0.1.0 带上）

- Status: in_progress

## Goal

照 Zed 的 `auto_update`：启动器自己发现最新的正式版，后台下载，下载好后左下角导航块右边出现一个「重启以更新」按钮，点了就换成新版本。不打扰，不弹窗。

## 冻结的决策

1. **问 Worker 要最新版**：取 `https://launcher.lumilio.org/releases/latest/download/SHA256SUMS.txt`，失败就取 GitHub 原址 `https://github.com/EdwinZhanCN/LumilioCL/releases/latest/download/SHA256SUMS.txt`。这个文件每个 release 都有，文件名里带版本号（`LumilioCL-<version>-<os>-<arch>…`），按本机 OS/架构找到对应那一行，就得到版本、文件名和 sha256，不需要新的清单或 Worker 路由。版本比当前大才下载。GitHub `latest` 不含 pre-release，所以只有正式版会被推送。
2. **下载**：`/releases/download/v<version>/<file>`，同样先 Worker 后 GitHub，HTTPS 流式写到数据目录 `updates/`，完成后对一下 SHA256SUMS 里的 sha256（反正已经拿到了），不对就删掉重下。其余信任交给 HTTPS，和 Zed 一样。
3. **节奏**：启动后检查一次，之后每小时一次（同 Zed）；「设置 → 关于」有「检查更新」。设置项 `auto_update`（默认开），关掉后只剩手动检查。
4. **安装**（照 Zed）：
   - macOS：`hdiutil attach -nobrowse` 挂载 dmg，`ditto` 把新的 `LumilioCL.app` 覆盖到正在运行的 app 上，卸载；重启用 `open -n`。
   - Linux：只管 `install.sh` 装的那份，把 tar.gz 解到 `~/.local/lib/lumiliocl/` 覆盖；重启直接起新二进制。
   - Windows：退出时以 `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS` 运行下载好的 Inno 安装器，`.iss` 的 `[Run]` 加一条静默安装后重新打开 `lumiliocl.exe`。
5. **不自动更新的构建**：构建时设了 `LUMILIO_UPDATE_EXPLANATION`（对应 Zed 的 `ZED_UPDATE_EXPLANATION`）就完全不检查，手动检查只显示这句话。deb 和 Windows 便携 zip 由 xtask 打包时设上；debug 构建也不检查。
6. **入口与游戏运行时不重启**：只在新版本下载好后出现一个橙色图标键，位于左下角「‹ › 页面标题」导航块的右边、中间导航坞的左边，与导航块同高；提示「新版本 X.Y.Z 已就绪，点击重启并更新」。有 `LaunchStatus::Running` 的会话时置灰，提示「游戏正在运行，游戏结束后再重启更新」。

## Tasks

- [x] T1：新 crate `crates/lumilio-updater`：解析 SHA256SUMS 找本机资源、SemVer 比较、Worker → GitHub 回退、流式下载与 SHA-256 校验；启动检查并每小时轮询。单元测试覆盖解析、版本比较、异常清单和回退。
- [x] T2：macOS app bundle、Linux 用户级 install.sh 和 Windows Inno 安装器安装与重启；Windows 静默安装后重新打开启动器。Linux 解包只取当前归档中的可执行文件，并在原路径原子替换。
- [x] T3：`LUMILIO_UPDATE_EXPLANATION`：xtask 给 deb 和便携 ZIP 构建单独的禁用自动更新版本；debug 和不受支持的安装位置显示原因，不执行检查。
- [x] T4：设置与界面：旧设置默认开启 `auto_update`；关于页显示版本、状态、手动检查和开关；校验下载完成后，左下角导航块右边出现橙色更新键，游戏运行时禁用。中英 `.ftl` 和 IA 注释已添加。
- [x] T5：生成 IA 路径并跑 `just check`；维护者于 2026-10-10 报告检查和界面目视验证均通过。
- [ ] T6：下一个版本发布时，Edwin 在自己的 Mac 上从 0.1.0 自动更新一次。

已实现但仍待真实发布验收的部分：macOS/Windows/Linux 安装器变体需要在各自原生 runner 上构建；维护者需在下一正式版从 0.1.0 实测覆盖更新。

## 风险

- macOS app 只有 ad hoc 签名：自己下载的文件不带 quarantine，应当不会再弹 Gatekeeper；文件访问等授权可能要重新给一次。Edwin 更新那次顺便看一眼。
- 地址写死在 0.1.0 里：`launcher.lumilio.org` 和 GitHub 仓库名要一直有效，GitHub 回退兜底。
- 发布时文件名格式和 SHA256SUMS.txt 不能变，否则老版本找不到更新。

## 实施记录

- 2026-10-10：Edwin 选了 Zed 那样的简单做法：不签清单、不做回滚和多机演练；Cloudflare Worker 已部署。
- 2026-10-10：实现落地在 `lumilio-updater`、app 更新 worker、设置/导航界面和 xtask 的平台专用构建。参考 Zed `crates/auto_update/src/auto_update.rs` 的轮询体验；没有复制代码。静默重启使用 Inno Setup `WizardSilent` 检查。
- 2026-10-10：`cargo test -p lumilio-updater --offline` 通过（5 项）；维护者接手 IA 生成、`just check` 和设置页目视检查。
- 2026-10-10：维护者报告 IA 生成、`just check` 与设置页目视检查全部通过。
- 外部参考：[Zed auto_update 源码](https://github.com/zed-industries/zed/blob/main/crates/auto_update/src/auto_update.rs)、[Inno Setup `WizardSilent`](https://jrsoftware.org/ishelp/topic_isxfunc_wizardsilent.htm)。
