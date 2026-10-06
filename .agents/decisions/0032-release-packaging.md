# 0032 — 发布：SemVer、三平台原生打包、草稿 release

- Status: accepted
- Date: 2026-10-06

## 背景

`assets/icons/PACKAGING.md` 写好了打包约定：平台身份、图标资产，以及 macOS / Windows / Linux 各自的产物和验收要求。它要求版本号只有一个来源，每个平台在本平台上原生构建，产物带上版本、OS、架构、commit 和 SHA-256。在这之前仓库里只有一份 CI，跑在 macOS 上；app 的 Wayland `app_id` 还是占位的 `dev.lumilio.launcher`，可执行文件名叫 `lumilio-app`，也没有任何打包代码。

## 决定

- **版本**：工作区 `[workspace.package] version` 是唯一来源，遵循 SemVer 2.0.0。各平台需要的版本字段都从它派生：macOS 和 Windows 只接受数字，取 `MAJOR.MINOR.PATCH`；Debian 的预发布版本写成 `0.2.0~beta.1`，保证能按顺序升级到 `0.2.0`。推送 `v<version>` 标签触发发布，标签和版本不一致时工作流直接失败；带 `-` 的版本标为 pre-release。首个版本是 `0.1.0`：1.0 之前，minor 版本可以有不兼容的改动。
- **身份**：`APP_ID = app.lumilio.LumilioCL`。正式构建用它，debug 构建用 `.dev` 后缀。可执行文件改名为 `lumiliocl`，crate 名仍是 `lumilio-app`。Inno 的 AppId 定为 `{6927ECC0-…}`，以后不再改。xtask 的测试检查这些标识在 main.rs、`.desktop`、`install.sh` 和 `.iss` 里是否一致。
- **打包放在 xtask 里**：用 Rust 写的 `crates/lumilio-xtask`（`cargo xtask package` / `just package`），不用每个平台各写一份 shell 或 PowerShell 脚本。这样三个平台共用一份代码，clippy 和单元测试能覆盖到 Info.plist、deb control、版本推导、zip 和校验和。产物：macOS 出 `.dmg`，图标由 actool 从 `AppIcon.icon` 编译成 `Assets.car` 加 `.icns`；Windows 出 Inno 安装器（按用户安装，快捷方式带 AppUserModelID）和便携 ZIP；Linux 出 `.tar.gz` 加 `install.sh`，以及 `.deb`（`dpkg-shlibdeps` 推导依赖，另外手动补上运行时才加载的 `libvulkan1`）。
- **签名**：签名可选，由仓库 secrets 打开。没有 secrets 时照常出包：macOS 用 ad hoc 签名，Windows 不签。每个包的 `BUILD.txt` 写明实际的签名状态。维护者决定不购买证书，README 说明首次打开时怎么通过 Gatekeeper 和 SmartScreen，也说明怎么用 `SHA256SUMS.txt` 校验下载的文件；签名的代码路径仍然保留，以后可以启用。工作流只创建草稿 release，按 PACKAGING.md §5 对下载后的产物验收，通过后由人发布。
- **构建平台**：macOS 用 `macos-26`（actool 需要 Xcode 26），产物为 arm64；Windows 用 `windows-2022`，产物为 x64；Linux 在 `ubuntu:22.04` 容器里构建 x64，glibc 2.35 是能运行的下限。

## 后果

- 正面：一条命令出包，本机和 CI 用的是同一份代码；版本、身份、AppId 写错时测试或标签检查会失败，不会带进发布。
- 代价：签名和公证所需的证书与凭据要另外准备，没准备之前的版本会触发 Gatekeeper 或 SmartScreen 提示。Linux 和 Windows 的代码只在 release 工作流里编译，日常 CI 仍然只跑 macOS，所以这两个平台的编译问题要到出包时才会暴露。暂时不出 macOS x86_64、RPM、Arch、Flatpak 和 AppImage。Windows 的「便携」只是免安装，数据仍然写在 `%APPDATA%\LumilioCL`。
