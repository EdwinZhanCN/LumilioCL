---
title: 安装 LumilioCL
description: 选择安装包，核对下载文件，安装启动器。
---

## 选择安装包

打开[官方 Releases 页面](https://github.com/EdwinZhanCN/LumilioCL/releases/latest)。
选择适合操作系统的安装包。
把示例中的 `<version>` 替换为发布版本。
窄屏下可横向滚动表格。

| 系统 | 安装包 | 要求 |
| --- | --- | --- |
| macOS | `LumilioCL-<version>-macos-arm64.dmg` | Apple silicon；macOS 11 或更新版本 |
| Windows | `LumilioCL-<version>-windows-x64-setup.exe` | Windows 10 或 11；64 位 |
| Windows 免安装版 | `LumilioCL-<version>-windows-x64-portable.zip` | Windows 10 或 11；64 位 |
| Debian 或 Ubuntu | `LumilioCL-<version>-linux-x64.deb` | Debian 12 或 Ubuntu 22.04 或更新版本；64 位；Vulkan 驱动 |
| 其他 Linux 系统 | `LumilioCL-<version>-linux-x64.tar.gz` | 64 位；兼容的系统库；Vulkan 驱动 |

macOS 安装包不支持 Intel 处理器。

## 核对下载文件

1. 从同一个 Release 下载 `SHA256SUMS.txt`。
2. 使用下方对应命令计算安装包的校验值。
3. 对比结果与 `SHA256SUMS.txt` 中的对应条目。
4. 如果值不同，停止安装。

macOS：

```sh
shasum -a 256 LumilioCL-<version>-macos-arm64.dmg
```

Linux：

```sh
sha256sum LumilioCL-<version>-linux-x64.deb
```

Windows PowerShell：

```powershell
Get-FileHash .\LumilioCL-<version>-windows-x64-setup.exe -Algorithm SHA256
```

校验值相同，表示文件与发布的校验值一致。
这不证明发布者的身份。

## 在 macOS 安装

1. 打开 `.dmg` 文件。
2. 把 LumilioCL 拖入 Applications。
3. 从 Applications 打开 LumilioCL。

未签名的安装包可能触发安全提示。
继续前请核对发布来源。
如果系统阻止打开应用，请阅读[首次打开时的系统提示](#首次打开时的系统提示)。

## 在 Windows 安装

1. 打开 `setup.exe` 安装包。
2. 按安装器提示操作。
3. 打开 LumilioCL。

安装器不需要管理员权限。
使用免安装版时，先解压 ZIP 文件。
然后打开解压目录中的 `lumiliocl.exe`。

## 在 Linux 安装

Debian 安装包：在下载目录运行以下命令。

```sh
sudo apt install ./LumilioCL-<version>-linux-x64.deb
```

tar 安装包：

1. 解压 `.tar.gz` 文件。
2. 在解压目录打开终端。
3. 运行 `./install.sh`。
4. 打开 LumilioCL。

tar 安装器只为当前用户安装。
它不需要 root 权限。
运行 `./install.sh --uninstall` 可移除该安装。

## 首次打开时的系统提示

先确认安装包来自本仓库的 GitHub Releases。
未签名或缺少系统信任记录的安装包，可能触发 macOS Gatekeeper 或 Windows SmartScreen 提示。
这类提示不直接证明文件恶意，也不能证明文件安全。
继续前请核对发布来源和 SHA-256。

### macOS

1. 在 Applications 里双击 LumilioCL。弹出「无法打开 "LumilioCL"」时，点**完成**，不要点「移到废纸篓」。
2. 打开**系统设置 → 隐私与安全性**，滑到页面最下方，可以看到「已阻止打开 LumilioCL」。
3. 点旁边的**仍要打开**，输入电脑的开机密码。
4. 在再次弹出的窗口里点**打开**。

之后双击就能正常打开。

macOS 14 或更早的系统：按住 Control 键点 LumilioCL（或右键点它），选**打开**，再点一次**打开**。

### 提示「已损坏，无法打开」

该提示也可能由下载损坏或安全校验失败引起。
请先从官方 Releases 重新下载，并核对 SHA-256。
确认来源可信、问题确实来自隔离标记后，才在「终端」运行以下命令：

```sh
xattr -dr com.apple.quarantine /Applications/LumilioCL.app
```

然后再双击打开。

### Windows

1. 浏览器提示「不常下载」时：在 Edge 里点该下载右边的 **…** → **保留** → **仍然保留**。
2. 出现蓝色窗口「Windows 已保护你的电脑」时：点**更多信息**，再点**仍要运行**。

### Linux

系统提示取决于发行版与桌面环境，请按上文的步骤安装。

## 保留游戏数据

移除应用不会删除游戏或存档。
默认数据目录如下。

| 系统 | 目录 |
| --- | --- |
| macOS | `~/Library/Application Support/LumilioCL` |
| Windows | `%APPDATA%\LumilioCL` |
| Linux | `~/.local/share/lumilio` |

修改或删除游戏数据前，请备份重要存档。
