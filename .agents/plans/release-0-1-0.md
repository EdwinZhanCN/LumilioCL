# 发布 0.1.0

- Status: in_progress

## Goal

`v0.1.0` 的草稿 release 里有 macOS、Windows、Linux 三个平台的包。每个下载下来的包都按 PACKAGING.md §5 验收通过后，再发布这个 release。打包方式的决策见 ADR 0032。

## Scope

- 包括：xtask 打包、release 工作流、平台身份、图标、自动更新（见 `auto-update.md`），以及首个标签。
- 不包括：macOS x86_64、RPM / Arch、Flatpak / AppImage、真正的便携数据目录。

## Tasks

- [x] 图标目录移到 `assets/icons/`；手写 `macos/AppIcon.icon`，用 Xcode 26.6 的 actool 编译验证，产出 `Assets.car` 和 `AppIcon.icns`
- [x] `lumilio-app`：可执行文件改名 `lumiliocl`，设置 `APP_ID` 和 `set_app_identity`，Windows 用 build.rs 嵌入资源，release 构建不弹控制台窗口
- [x] `crates/lumilio-xtask`、`just package`、`.github/workflows/release.yml`
- [x] CI 在 Linux 上统一运行 `just ci`；平台原生打包留在 release workflow，移除单独的 Windows cubiomes 编译 job（ADR 0043）
- [x] 自动更新实现进入 0.1.0：正式版检查、校验下载、平台安装和重启入口；各安装方式的支持范围见 `auto-update.md` 和 ADR 0042
- [x] 本机 macOS 打包演练（ad hoc 签名）：DMG 25 MB；Info.plist、`Assets.car`、签名验证和 SHA-256 都正确；从 DMG 复制出来的 app 能启动到首页
- [ ] 在 Icon Composer 里检查 `AppIcon.icon` 的深色外观，以及 26 和 27 两代渲染
- [ ] 手动跑 release 工作流演练（workflow_dispatch），确认 Windows 和 Linux 都能编译出包。首次演练中 Windows 的 Rust 编译成功，但 ISCC 拒绝 `\\?\D:` 扩展路径前缀；已在传参边界做路径规范化，待 CI 重跑验收。
- [x] 签名：维护者决定不签名。README（中英文）用最直白的话解释首次打开时的警告，并写明 macOS、Windows 上怎么放行
- [ ] 打 `v0.1.0` 标签，等草稿 release 生成
- [ ] 验收下载后的产物（PACKAGING.md §5 的五项），通过后发布
- [ ] 下一正式版发布后，在维护者的 Mac 上从 0.1.0 实测自动更新（`auto-update.md` T5）

## Open questions

- deb 的 `Maintainer` 现在填的是 GitHub noreply 地址（`linux.rs` 的 `MAINTAINER`），需要维护者确认。
- Windows release 构建没有控制台窗口，「数据文件夹被占用」之类的启动错误只写到 stderr，用户看不到。需要一个看得见的报错方式。
