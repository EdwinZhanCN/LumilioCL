# 发布 0.1.0

- Status: in_progress

## Goal

`v0.1.0` 的草稿 release 里有 macOS、Windows、Linux 三个平台的包。每个下载下来的包都按 PACKAGING.md §5 验收通过后，再发布这个 release。打包方式的决策见 ADR 0032。

**0.1.0 必须带完整的自动更新**（检查、后台下载、校验、安装、重启、回滚），没有就不发布。详细计划见 `auto-update.md`；本计划剩下的未完成项已并入它的阶段 D、E。

## Scope

- 包括：xtask 打包、release 工作流、平台身份、图标、完整的自动更新（`auto-update.md`），以及首个标签。
- 不包括：macOS x86_64、RPM / Arch、Flatpak / AppImage、真正的便携数据目录。

## Tasks

- [x] 图标目录移到 `assets/icons/`；手写 `macos/AppIcon.icon`，用 Xcode 26.6 的 actool 编译验证，产出 `Assets.car` 和 `AppIcon.icns`
- [x] `lumilio-app`：可执行文件改名 `lumiliocl`，设置 `APP_ID` 和 `set_app_identity`，Windows 用 build.rs 嵌入资源，release 构建不弹控制台窗口
- [x] `crates/lumilio-xtask`、`just package`、`.github/workflows/release.yml`
- [x] 本机 macOS 打包演练（ad hoc 签名）：DMG 25 MB；Info.plist、`Assets.car`、签名验证和 SHA-256 都正确；从 DMG 复制出来的 app 能启动到首页
- [ ] 在 Icon Composer 里检查 `AppIcon.icon` 的深色外观，以及 26 和 27 两代渲染（`auto-update.md` T19）
- [ ] 提交并推送后，先手动跑一次 release 工作流演练（workflow_dispatch），确认 Windows 和 Linux 都能编译出包（T21）
- [x] 签名：维护者决定不签名。README（中英文）用最直白的话解释首次打开时的警告，并写明 macOS、Windows 上怎么放行
- [ ] 自动更新：`auto-update.md` 阶段 A–C
- [ ] 先发 `v0.1.0-rc.1`、`v0.1.0-rc.2` 两个 pre-release，三平台演练 rc.1 → rc.2 自动更新（T22、T23）
- [ ] 打 `v0.1.0` 标签，等草稿 release 生成，`just sign-release` 签清单
- [ ] 验收下载后的产物（PACKAGING.md §5 的六项），先以 pre-release 发布并从 rc.2 升级验证，再改为正式版（T24）

## Open questions

- deb 的 `Maintainer`（T18） 现在填的是 GitHub noreply 地址（`linux.rs` 的 `MAINTAINER`），需要维护者确认。
- Windows release 构建没有控制台窗口，「数据文件夹被占用」之类的启动错误只写到 stderr，用户看不到。需要一个看得见的报错方式：计划用 `MessageBoxW`（T17）。
