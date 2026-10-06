<div align="center">

<img src="assets/icons/linux/hicolor/256x256/apps/app.lumilio.LumilioCL.png" width="128" height="128" alt="LumilioCL 图标">

# LumilioCL

一个用 Rust 写的原生 Minecraft 启动器。

[![CI](https://github.com/EdwinZhanCN/LumilioCL/actions/workflows/ci.yml/badge.svg)](https://github.com/EdwinZhanCN/LumilioCL/actions/workflows/ci.yml)
[![最新版本](https://img.shields.io/github/v/release/EdwinZhanCN/LumilioCL?include_prereleases&sort=semver&label=%E6%9C%80%E6%96%B0%E7%89%88%E6%9C%AC)](https://github.com/EdwinZhanCN/LumilioCL/releases/latest)
[![许可证](https://img.shields.io/badge/license-AGPL--3.0--only-blue)](LICENSE)
![平台](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey)

![Rust](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust)
![GPUI](https://img.shields.io/badge/UI-GPUI-4F46E5)
![gpui-component](https://img.shields.io/badge/gpui--component-0.7-4F46E5)
![Tokio](https://img.shields.io/badge/async-Tokio-463D3B)
![SQLite](https://img.shields.io/badge/storage-SQLite-003B57?logo=sqlite&logoColor=white)
![wgpu](https://img.shields.io/badge/3D_preview-wgpu-2B6CB0)

简体中文 · [English](README.en.md)

</div>

## 简介

LumilioCL 用 Rust 编写，界面基于 [GPUI](https://www.gpui.rs/) 和
[gpui-component](https://github.com/longbridge/gpui-component)，不依赖 Electron 或网页技术。

- **游戏库**：新建原版、Fabric 或 Quilt 游戏，用收藏、合集和搜索管理。可以导入 `.mrpack`
  和 MultiMC / Prism 整合包，也可以把其他启动器（MultiMC、Prism、`.minecraft`）里的游戏搬
  过来，原文件不动。
- **发现**：浏览并安装 Modrinth 上的模组、资源包、光影和整合包，按游戏版本和加载器筛选。
- **Java**：自动下载并管理每个游戏需要的 Java，不用自己装。
- **账户**：离线账户（可以换皮肤），以及 LittleSkin 等第三方认证服务器。
- **游戏页**：管理内容、世界和服务器，查看截图和启动历史，做完整备份和恢复。游戏崩溃时，
  诊断页会说明可能的原因。
- **插件**：崩溃分析、Litematica 投影（材料清单和 3D 预览）、Discord 状态显示（默认关闭）。

> [!NOTE]
> Microsoft 正版登录的流程已经做好，但这个启动器的应用注册还在等 Mojang 审批。审批通过之前，
> 登录会在最后一步失败，界面会说明原因。这段时间可以先用离线账户或第三方登录。

## 获取

到 [Releases](https://github.com/EdwinZhanCN/LumilioCL/releases/latest) 下载对应系统的文件：

| 系统 | 下载哪个 | 要求 |
| --- | --- | --- |
| macOS | `LumilioCL-<版本>-macos-arm64.dmg` | Apple 芯片（M1 及更新），macOS 11 或更新；暂不支持 Intel Mac |
| Windows | `LumilioCL-<版本>-windows-x64-setup.exe`（安装版，推荐）<br>`LumilioCL-<版本>-windows-x64-portable.zip`（免安装，解压即用） | Windows 10 / 11，64 位 |
| Linux（Debian、Ubuntu 等） | `LumilioCL-<版本>-linux-x64.deb` | 64 位，Ubuntu 22.04 / Debian 12 或更新，需要能用的 Vulkan 显卡驱动 |
| Linux（其他发行版） | `LumilioCL-<版本>-linux-x64.tar.gz` | 同上 |

安装方法：

- **macOS**：打开 `.dmg`，把 LumilioCL 拖进「应用程序」文件夹。
- **Windows**：双击 `setup.exe` 一路下一步，不需要管理员权限。免安装版解压到任意位置后，
  双击 `lumiliocl.exe`。
- **Linux `.deb`**：在下载目录运行 `sudo apt install ./LumilioCL-<版本>-linux-x64.deb`。
- **Linux `.tar.gz`**：解压后运行里面的 `./install.sh`，只装给当前用户，不需要 root。卸载时
  运行 `./install.sh --uninstall`。

卸载 LumilioCL 不会删除你的游戏和存档，它们保存在这些位置：macOS 是
`~/Library/Application Support/LumilioCL`，Windows 是 `%APPDATA%\LumilioCL`，Linux 是
`~/.local/share/lumilio`。

## 第一次打开时系统弹出警告？

**这是正常的，只需要处理一次。**

Apple 和微软会给付费购买了「开发者证书」的软件做标记，系统认得这个标记，就不会提醒。
LumilioCL 是免费的个人项目，没有购买证书，所以 macOS 和 Windows 第一次打开时会提醒你
「无法确认开发者」。这只说明系统不认识开发者，**不代表软件有问题**。LumilioCL 的代码全部公开，
安装包由 GitHub 根据公开的代码自动构建。

### macOS

1. 在「应用程序」里双击 LumilioCL。弹出「无法打开 "LumilioCL"」时，点 **完成**，
   不要点「移到废纸篓」。
2. 打开 **系统设置 → 隐私与安全性**，滑到页面最下方，可以看到「已阻止打开 LumilioCL」。
   点旁边的 **仍要打开**，输入电脑的开机密码。
3. 在再次弹出的窗口里点 **打开**。

之后它就和普通应用一样，双击就能打开。

<details>
<summary>macOS 14 或更早的系统</summary>

在「应用程序」里按住 Control 键点 LumilioCL（或者右键点它），选 **打开**，再点一次 **打开**。

</details>

<details>
<summary>提示「已损坏，无法打开」</summary>

这是 macOS 对从网上下载的未签名应用的另一种拦截方式，软件本身没有损坏。打开「终端」，粘贴
下面这行，按回车：

```sh
xattr -dr com.apple.quarantine /Applications/LumilioCL.app
```

然后再双击打开。

</details>

### Windows

1. 如果浏览器提示「不常下载」：在 Edge 里点该下载右边的 **…** → **保留** → **仍然保留**。
2. 运行时如果出现蓝色窗口「Windows 已保护你的电脑」：点 **更多信息**，再点 **仍要运行**。

### Linux

不会弹警告，按上面的方法安装即可。

### 想确认下载的文件没被改过？

每个 Release 都附有 `SHA256SUMS.txt`，里面是每个文件的「指纹」。把它和安装包放在同一个
文件夹，然后运行：

- macOS：`shasum -a 256 -c SHA256SUMS.txt --ignore-missing`
- Linux：`sha256sum -c SHA256SUMS.txt --ignore-missing`
- Windows（PowerShell）：`Get-FileHash .\文件名 -Algorithm SHA256`，再把结果和
  `SHA256SUMS.txt` 里对应的那一行比对。

显示 `OK`，或者两串字符完全一样，就说明文件和 GitHub 构建出来的一模一样。

## 参与开发

```sh
cargo run -p lumilio-app   # 运行
just check                 # 构建、测试、clippy、rustfmt，和 CI 一致
just package               # 为当前平台打包到 dist/
```

- 工作区：`lumilio-core`（与界面无关的启动器逻辑）、`lumilio-ui`（GPUI 界面）、`lumilio-app`
  （进程启动和组装）、`lumilio-schematic-render`（原生投影渲染）、`lumilio-plugin-*`（插件），
  以及 `lumilio-xtask`（发布打包）。
- 用 `LUMILIO_HOME=/tmp/<任意文件夹>` 换一个一次性的数据目录。
- 启动器现在能做什么，见从代码生成的 [`docs/ia/paths/`](docs/ia/paths/README.md)；正在进行的
  工作在 [`.agents/plans/`](.agents/plans/README.md)；界面、动效和文案遵循
  [`docs/design-language.md`](docs/design-language.md)。
- 发布：推送和工作区版本一致的 `v<版本>` 标签（SemVer），会在三个平台上打包并创建草稿
  Release，详见 [`assets/icons/PACKAGING.md`](assets/icons/PACKAGING.md) 和 ADR 0032。

<details>
<summary>界面审阅用的环境变量（只影响当次运行）</summary>

- `LUMILIO_PAGE` 直接打开某个页面，例如 `library:2`、`discover:1`、`activity`、
  `instance:1:4:1`（编号、分区、子标签，从 0 开始）。
- `LUMILIO_INSTANCE=<id>`（配合 `LUMILIO_INSTANCE_TAB`、`LUMILIO_INSTANCE_GROUP`）和
  `LUMILIO_PROJECT=mod/sodium` 打开某个游戏或项目详情。
- `LUMILIO_REVIEW_APPEARANCE=light|dark`、`LUMILIO_REVIEW_MIN=1`（720×480）、
  `LUMILIO_REVIEW_STILL=1`（减少动效）。

</details>

## 许可证

LumilioCL 以 GNU Affero 通用公共许可证第 3 版（`AGPL-3.0-only`）发布，见 [`LICENSE`](LICENSE)。
改编自上游的代码，都在所在文件或函数的注释里写明了来源和许可证。内置字体和图标的许可证见
[`crates/lumilio-ui/assets/ATTRIBUTIONS.md`](crates/lumilio-ui/assets/ATTRIBUTIONS.md)，
原生渲染依赖的许可证见 [`ATTRIBUTIONS.md`](ATTRIBUTIONS.md)。

## 致谢

- **[Modrinth App](https://github.com/modrinth/code)**：它的 Rust 启动器后端 `app-lib`，是
  LumilioCL 实现认证、Java 运行时、实例、整合包和进程管理时首先参考的地方。部分代码改编自它
  （GPL-3.0-only）。LumilioCL 没有使用 Modrinth 的品牌。
- **[HMCL](https://github.com/HMCL-dev/HMCL)**：在功能广度、平台差异和各种边界情况上，
  它是 LumilioCL 最主要的参考。部分代码改编自它（GPL-3.0-or-later）。
- **[Nucleation](https://github.com/Schem-at/Nucleation)**（Schem-at，MIT）：Litematica 投影的
  解析、网格和 GPU 渲染。
- **[Schematic-Mesher](https://github.com/Schem-at/Schematic-Mesher)**（Schem-at，AGPL-3.0-only）：
  方块几何、液体和箱子等的网格生成。

后两个项目以可编辑源码快照的形式维护在 [`forks/`](forks/README.md)，那里记录了基线版本和本地改动。
感谢这些项目的作者和贡献者。
