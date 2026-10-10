# LumilioCL 打包约定

范围：gpui-kit 应用在 macOS / Windows / Linux 上的图标与发布产物。这里只写不变量和关键接口，具体脚本由 `cargo xtask` 实现（`crates/lumilio-xtask`，`just package`）；发布流程是 `.github/workflows/release.yml`，决策见 ADR 0032。

## 0. 身份（全平台唯一，发布后不可改）

- `APP_ID = app.lumilio.LumilioCL`。如果你不控制 `lumilio.app` 域名，在第一次发布前换掉。ID 里不要带 `-`。
- `APP_ID` 用在以下位置：
  - macOS：`CFBundleIdentifier`
  - GPUI：`cx.set_app_identity(APP_ID, "LumilioCL")`
  - Linux：`WindowOptions.app_id`、`.desktop` 文件名、`Icon=`、`StartupWMClass=`、hicolor 图标文件名
- Windows 安装器的 `AppId` 另取一个稳定值，之后永远不改：`{6927ECC0-2947-4CDB-8209-9B6142815660}`，写在 `crates/lumilio-xtask/packaging/LumilioCL.iss`，xtask 的测试守着它。
- 版本号只有一个来源，就是 `Cargo.toml` 的 `version`。以下字段都从它派生：Info.plist 的 `CFBundleShortVersionString` 和 `CFBundleVersion`、Inno 的 `AppVersion`、deb 的 `Version`、exe 的 VERSIONINFO（winresource 会自动读取）。
- 开发构建使用 `APP_ID.dev`，避免和正式版共用设置、通知和图标缓存。

## 1. 图标资产

- 本目录在仓库中的位置是 `assets/icons/`。
- 唯一事实来源是 `src/` 下的 SVG。
- 生成物包括 `windows/LumilioCL.ico` 和 `linux/hicolor/**`。规则是：改 SVG，运行 `python3 gen_icons.py [APP_ID]`，然后提交。不要手改生成物。CI 不运行生成器。
- 16 / 20 / 24 / 32 px 各有像素对齐母版（`tile-{N}.svg`），其他尺寸从大母版渲染。
- macOS 的源是 `macos/AppIcon.icon`，是 Icon Composer 文档，提交进仓库。它由 `src/macos/AppIcon-layers/` 的图层组成（`Assets/` 里是副本），按下列设置写成；改图层后在 Icon Composer 里打开它更新：
  - 背景：Fill = Solid `#E4E3DE`；Dark 外观下为 `#1F1F1D`。
  - 分组：每个 SVG 一组，按文件名序号由下到上，总共不超过 4 组。
  - Dark 外观：`1-keys` 改为 `#E4E3DE`，`0-slots` 改为 `#3B3A37`，`2-light` 不变。
  - 材质：`0-slots` 和 `1-keys` 关闭或调低 Liquid Glass，保持 TE 的哑光感。`2-light` 保留高光。`3-screws` 是可选组。
  - 在 Icon Composer 里同时检查 26 和 27 两代渲染。
- macOS 图层里不画机身、底边和圆角。形状、阴影、材质都由系统提供。

## 2. macOS

- 产物：`LumilioCL.app`，装进 `.dmg`，经过 Developer ID 签名、公证和 staple。
- 图标编译需要完整的 Xcode 26 或更高版本（Command Line Tools 里没有 actool），并且要在 macOS 26 或更高版本上运行：
  ```sh
  xcrun actool AppIcon.icon --compile "$APP/Contents/Resources" \
    --app-icon AppIcon --platform macosx --target-device mac \
    --minimum-deployment-target "$MIN_MACOS" \
    --output-partial-info-plist build/AppIcon.partial.plist
  ```
  这一步输出 `Assets.car` 供 macOS 26+ 使用，同时输出 `AppIcon.icns` 作为旧系统的回退。
- Info.plist 必须同时包含两项：`CFBundleIconName = AppIcon` 和 `CFBundleIconFile = AppIcon`。
- 不变量（顺序固定）：
  1. 资源完整放进 bundle，包括 `Assets.car`。
  2. 由内向外签名，开启 hardened runtime。
  3. 用 `codesign --verify` 验证。
  4. 打 DMG，再签 DMG。
  5. 公证，结果必须是 Accepted。
  6. staple。
  7. 重新下载 DMG，在干净机器上启动验证。
- 裸 `cargo run` 没有 bundle，Dock 里显示通用图标，这是预期行为。

## 3. Windows

- MSVC 工具链加 Windows SDK（需要 `rc.exe`）。`crates/lumilio-app/build.rs` 用 winresource 嵌入图标和 VERSIONINFO。
  不要在这里设置 manifest。GPUI 默认的 `windows-manifest` feature 已经通过 embed-resource 嵌入了一份。
- exe 内嵌的图标资源是唯一来源。不要依赖运行时设置窗口图标。
- 产物有两种：
  - 便携 ZIP：先签名的 exe 再加运行时文件。
  - Inno Setup 安装器：`SetupIconFile` 和 `UninstallDisplayIcon` 都指向同一个图标。
- 安装器构建支持自动更新；便携 ZIP 的构建会禁用自动更新，因为它没有可安全覆盖的安装位置。
- 签名顺序：先签 exe，再打包，最后签 setup.exe。卸载器的签名按 Inno 的 SignedUninstaller 流程处理。
- 验收：在干净的 VM 上，以 100% 和 200% 缩放分别检查标题栏、任务栏、开始菜单和资源管理器里的图标。

## 4. Linux

- 运行时：`WindowOptions { app_id: Some(APP_ID.into()), .. }`。缺少它时，Wayland 无法把窗口关联到 `.desktop`，任务栏会显示通用图标。
- 安装布局：
  ```
  /usr/lib/lumiliocl/lumiliocl
  /usr/share/applications/<APP_ID>.desktop
  /usr/share/icons/hicolor/<N>x<N>/apps/<APP_ID>.png
  /usr/share/icons/hicolor/scalable/apps/<APP_ID>.svg
  ```
- 产物：
  - `.tar.gz` 加 `install.sh`，安装到用户级 XDG 目录。
  - `.deb`，`Depends` 由 `dpkg-shlibdeps` 推导，不要从别的应用复制。
  - RPM 和 Arch 包复用同一棵 staging 树。
- `.tar.gz` 安装到 `~/.local/lib/lumiliocl/` 后支持自动更新；`.deb` 构建会禁用自动更新，升级交给系统包管理器。
- 在你支持的最老发行版上构建。运行需要可用的 Vulkan 驱动。
- 发布前对 `.desktop` 文件运行 `desktop-file-validate`。
- Flatpak 和 AppImage 暂缓。启动器需要拉起 Java 进程、读写大量游戏目录，沙箱权限要单独设计。

## 5. CI 与验收

- 每个平台原生构建，不做交叉打包：
  - macOS 26 runner：arm64，按需加 x86_64。
  - Windows：x64。
  - Linux：用最老支持发行版的容器，x64。
- 发布产物必须带上：版本号、OS、架构、构建 commit，以及 SHA-256。文件名是 `LumilioCL-<version>-<os>-<arch>…`，commit 和签名状态写在每个包里的 `BUILD.txt`（macOS 还写进 Info.plist 的 `LumilioCLCommit`），校验和汇总为 `SHA256SUMS.txt`。
- 推送 `v<version>` 标签触发发布，标签必须等于工作区版本（SemVer；带 `-` 的预发布版本标为 pre-release）。工作流只建草稿 release，验收通过后由人发布。
- 验收必须针对下载后的产物，不是构建目录。检查五项：
  1. 首次启动正常。
  2. 图标在各处显示正确。
  3. 从 N 覆盖安装到 N+1 后，设置和快捷方式都还在。
  4. 卸载后只留下用户数据。
  5. 签名和公证状态有效。
