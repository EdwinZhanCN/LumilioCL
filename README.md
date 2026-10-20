<div align="center">

<img src="assets/icons/linux/hicolor/256x256/apps/app.lumilio.LumilioCL.png" width="128" height="128" alt="LumilioCL 图标">

# LumilioCL

原生、高性能、可扩展的 Minecraft 启动器，用 Rust + GPUI 写成，不依赖 Electron 或 WebView。

[![最新版本](https://img.shields.io/github/v/release/EdwinZhanCN/LumilioCL?include_prereleases&sort=semver&label=%E6%9C%80%E6%96%B0%E7%89%88%E6%9C%AC)](https://github.com/EdwinZhanCN/LumilioCL/releases/latest)
[![许可证](https://img.shields.io/badge/license-AGPL--3.0--only-blue)](LICENSE)
[![CI](https://github.com/EdwinZhanCN/LumilioCL/actions/workflows/ci.yml/badge.svg)](https://github.com/EdwinZhanCN/LumilioCL/actions/workflows/ci.yml)

简体中文 · [English](README.en.md)

<img src="assets/screenshots/home.png" width="720" alt="LumilioCL 首页">

</div>

游戏库与整合包导入、Modrinth / CurseForge 内容、自动管理 Java、世界地图、崩溃诊断和插件，功能介绍见[官网](https://launcher.lumilio.org/docs/guide/)。

## 下载

支持 macOS（Apple 芯片）、Windows 10 / 11（x64）和 Linux（x64）。

- [官网下载](https://launcher.lumilio.org/)（国内直连）或 [GitHub Releases](https://github.com/EdwinZhanCN/LumilioCL/releases/latest)
- 安装和校验 SHA-256：[安装指南](https://launcher.lumilio.org/docs/install/)

## 第一次打开时系统弹出警告？

请先确认安装包来自本仓库的 [GitHub Releases](https://github.com/EdwinZhanCN/LumilioCL/releases/latest)。完整说明见[官网](https://launcher.lumilio.org/docs/install/#首次打开时的系统提示)。

- **macOS**：弹出「无法打开」时点**完成**，再到**系统设置 → 隐私与安全性**最下方点**仍要打开**。macOS 14 及更早：按住 Control 键点应用，选**打开**。
- **macOS 提示「已损坏」**：先重新下载并核对 SHA-256；确认来源可信后，才在终端运行：

  ```sh
  xattr -dr com.apple.quarantine /Applications/LumilioCL.app
  ```

- **Windows**：Edge 提示「不常下载」时点 **…** → **保留** → **仍然保留**；出现「Windows 已保护你的电脑」时点**更多信息** → **仍要运行**。

## 从源码构建

```sh
cargo run -p lumilio-app   # 运行
just check                 # 构建、测试、clippy、rustfmt，和 CI 一致
just package               # 为当前平台打包到 dist/
```

开发环境和贡献规范见[参与开发](https://launcher.lumilio.org/docs/contribute/)，开发约定见 [`AGENTS.md`](AGENTS.md)。

## 许可证与致谢

以 [AGPL-3.0-only](LICENSE) 发布。部分代码改编自 [Modrinth App](https://github.com/modrinth/code)（GPL-3.0-only）和 [HMCL](https://github.com/HMCL-dev/HMCL)（GPL-3.0-or-later），投影渲染基于 [Nucleation](https://github.com/Schem-at/Nucleation)（MIT）和 [Schematic-Mesher](https://github.com/Schem-at/Schematic-Mesher)（AGPL-3.0-only），来源记在对应文件的注释里。字体、图标和渲染依赖的许可证见 [`ATTRIBUTIONS.md`](ATTRIBUTIONS.md) 与 [`crates/lumilio-ui/assets/ATTRIBUTIONS.md`](crates/lumilio-ui/assets/ATTRIBUTIONS.md)。LumilioCL 没有使用 Modrinth 的品牌。
LumilioCL is an independent, open-source Minecraft launcher.

NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.