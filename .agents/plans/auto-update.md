# 自动更新（0.1.0 带上）

- Status: planned

## Goal

照 Zed 的 `auto_update`：启动器自己发现最新的正式版，后台下载，标题栏出现「重启以更新」，点了就换成新版本。不打扰，不弹窗。

## 冻结的决策

1. **问 Worker 要最新版**：取 `https://launcher.lumilio.org/releases/latest/download/SHA256SUMS.txt`，失败就取 GitHub 原址 `https://github.com/EdwinZhanCN/LumilioCL/releases/latest/download/SHA256SUMS.txt`。这个文件每个 release 都有，文件名里带版本号（`LumilioCL-<version>-<os>-<arch>…`），按本机 OS/架构找到对应那一行，就得到版本、文件名和 sha256，不需要新的清单或 Worker 路由。版本比当前大才下载。GitHub `latest` 不含 pre-release，所以只有正式版会被推送。
2. **下载**：`/releases/download/v<version>/<file>`，同样先 Worker 后 GitHub，HTTPS 流式写到数据目录 `updates/`，完成后对一下 SHA256SUMS 里的 sha256（反正已经拿到了），不对就删掉重下。其余信任交给 HTTPS，和 Zed 一样。
3. **节奏**：启动后检查一次，之后每小时一次（同 Zed）；「设置 → 关于」有「检查更新」。设置项 `auto_update`（默认开），关掉后只剩手动检查。
4. **安装**（照 Zed）：
   - macOS：`hdiutil attach -nobrowse` 挂载 dmg，`ditto` 把新的 `LumilioCL.app` 覆盖到正在运行的 app 上，卸载；重启用 `open -n`。
   - Linux：只管 `install.sh` 装的那份，把 tar.gz 解到 `~/.local/lib/lumiliocl/` 覆盖；重启直接起新二进制。
   - Windows：退出时以 `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS` 运行下载好的 Inno 安装器，`.iss` 的 `[Run]` 加一条静默安装后重新打开 `lumiliocl.exe`。
5. **不自动更新的构建**：构建时设了 `LUMILIO_UPDATE_EXPLANATION`（对应 Zed 的 `ZED_UPDATE_EXPLANATION`）就完全不检查，手动检查只显示这句话。deb 和 Windows 便携 zip 由 xtask 打包时设上；debug 构建也不检查。
6. **游戏在运行时不重启**：有 `LaunchStatus::Running` 的会话时「重启以更新」置灰，说明「游戏结束后再更新」。

## Tasks

- [ ] T1：新 crate `crates/lumilio-updater`：解析 SHA256SUMS 找本机资源、版本比较、Worker → GitHub 回退、下载与 sha256 校验、每小时轮询；单元测试覆盖解析、版本比较和回退。
- [ ] T2：各平台安装与重启，`crates/lumilio-updater/src/install/{macos,linux,windows}.rs`；`crates/lumilio-xtask/packaging/LumilioCL.iss` 加静默安装后重启。
- [ ] T3：`LUMILIO_UPDATE_EXPLANATION`：`crates/lumilio-xtask/src/{linux,windows}.rs` 给 deb 和便携 zip 设上，updater 读到就不检查。
- [ ] T4：设置与界面：`LauncherSettings` 加 `auto_update`；`crates/lumilio-app/src/live/update.rs` 接状态；标题栏「重启以更新」按钮（游戏运行时置灰）、`pages/settings/about.rs` 的版本、状态和「检查更新」；中英 `.ftl`、`just ia`。
- [ ] T5：验证：`just check`；下一个版本发布时，Edwin 在自己的 Mac 上从 0.1.0 自动更新一次。

## 风险

- macOS app 只有 ad hoc 签名：自己下载的文件不带 quarantine，应当不会再弹 Gatekeeper；文件访问等授权可能要重新给一次。Edwin 更新那次顺便看一眼。
- 地址写死在 0.1.0 里：`launcher.lumilio.org` 和 GitHub 仓库名要一直有效，GitHub 回退兜底。
- 发布时文件名格式和 SHA256SUMS.txt 不能变，否则老版本找不到更新。

## 实施记录

- 2026-10-10：Edwin 选了 Zed 那样的简单做法：不签清单、不做回滚和多机演练；Cloudflare Worker 已部署。
