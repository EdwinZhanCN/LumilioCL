---
title: 开发环境
description: 准备 Rust 工作区和网站工具。
---

## 获取源码

安装 Git、Rustup 和 [just](https://just.systems/)。
使用操作系统的原生构建工具。
macOS 需要 Xcode Command Line Tools。
Windows 需要 Visual Studio C++ 构建工具。

如果没有仓库写权限，先创建 GitHub fork。
在下方克隆命令中使用 fork 的 URL。

1. 克隆仓库。
2. 打开仓库根目录。

```sh
git clone https://github.com/EdwinZhanCN/LumilioCL.git
cd LumilioCL
```

为修改创建分支。
前缀见[分支与合并](/docs/contribute/change-and-review/#分支与合并)：

```sh
git switch -c fix/<change-name>
```

安装 `rust-toolchain.toml` 指定的工具链：

```sh
rustup toolchain install
```

开发工具链由仓库指定。
不要自行替换 Rust 版本。

## 准备 Linux 库

Linux CI 使用以下软件包。

```sh
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  build-essential clang cmake pkg-config \
  libssl-dev libzstd-dev libfontconfig-dev libfreetype-dev \
  libvulkan-dev libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libx11-dev libx11-xcb-dev libxcb1-dev
```

运行启动器还需要桌面会话和图形驱动。
当前软件包列表见 [CI workflow](https://github.com/EdwinZhanCN/LumilioCL/blob/main/.github/workflows/ci.yml)。

## 运行启动器

在仓库根目录运行：

```sh
cargo run -p lumilio-app
```

手动测试使用临时数据目录。
启动应用前，把 `LUMILIO_HOME` 设为该目录。
破坏性测试不要使用重要游戏数据。

## 准备网站

安装 Node.js 22.12.0 或更新版本。
pnpm 版本使用 `web/package.json` 的 `packageManager` 字段。
在 `web/` 中运行 pnpm 命令。

```sh
cd web
pnpm install --frozen-lockfile
pnpm dev
```

打开 Astro 输出的本地 URL。
中文文档位于 `/docs/`，英文文档位于 `/en/docs/`。

## 选择检查

以下 recipe 在仓库根目录运行。

| 修改范围 | 迭代检查 | 交接前检查 |
| --- | --- | --- |
| Rust 代码 | `just test-pkg <crate> [filter]` | `just check` |
| 文档或 harness | 修改计划后运行 `just plans` | `just docs` |
| 网站或公开文档 | `just web` | `just web`；修改计划时还需 `just docs` |

`just check` 依次执行构建、测试、Clippy 和格式检查。
`just web` 执行依赖安装、Astro 检查、网站测试和站点构建。
命令成功结束前，不得声称检查已通过。
