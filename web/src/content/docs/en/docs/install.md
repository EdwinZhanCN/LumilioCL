---
title: Install LumilioCL
description: Select a package, check the download, and install the launcher.
---

## Select a package

Open the [official Releases page](https://github.com/EdwinZhanCN/LumilioCL/releases/latest).
Select the package for your operating system.
Replace `<version>` in the examples with the release version.
On a narrow screen, scroll the table to see all columns.

| System | Package | Requirements |
| --- | --- | --- |
| macOS | `LumilioCL-<version>-macos-arm64.dmg` | Apple silicon; macOS 11 or later |
| Windows | `LumilioCL-<version>-windows-x64-setup.exe` | Windows 10 or 11; 64-bit |
| Windows, portable | `LumilioCL-<version>-windows-x64-portable.zip` | Windows 10 or 11; 64-bit |
| Debian or Ubuntu | `LumilioCL-<version>-linux-x64.deb` | Debian 12 or Ubuntu 22.04 or later; 64-bit; Vulkan driver |
| Other Linux systems | `LumilioCL-<version>-linux-x64.tar.gz` | 64-bit; compatible system libraries; Vulkan driver |

The macOS package does not support Intel processors.

## Check the download

1. Download `SHA256SUMS.txt` from the same release.
2. Calculate the package checksum with the applicable command below.
3. Compare the result with the package entry in `SHA256SUMS.txt`.
4. Stop if the values differ.

Use this command on macOS:

```sh
shasum -a 256 LumilioCL-<version>-macos-arm64.dmg
```

Use this command on Linux:

```sh
sha256sum LumilioCL-<version>-linux-x64.deb
```

Use this command in Windows PowerShell:

```powershell
Get-FileHash .\LumilioCL-<version>-windows-x64-setup.exe -Algorithm SHA256
```

A matching checksum means that the file agrees with the published checksum.
It does not identify the publisher.

## Install on macOS

1. Open the `.dmg` file.
2. Drag LumilioCL into Applications.
3. Open LumilioCL from Applications.

An unsigned package can cause a security warning.
Check the release source before you continue.
If the system prevents the application from opening, use the [first-launch instructions](#warnings-on-first-launch).

## Install on Windows

1. Open the `setup.exe` package.
2. Follow the installer instructions.
3. Open LumilioCL.

The installer does not need administrator permissions.
For the portable package, extract the ZIP file.
Then open `lumiliocl.exe` from the extracted directory.

## Install on Linux

For a Debian package, run this command from the download directory:

```sh
sudo apt install ./LumilioCL-<version>-linux-x64.deb
```

For a tar package:

1. Extract the `.tar.gz` file.
2. Open a terminal in the extracted directory.
3. Run `./install.sh`.
4. Open LumilioCL.

The tar installer installs for the current user.
It does not need root permissions.
Run `./install.sh --uninstall` to remove that installation.

## Warnings on first launch

First verify that the package came from this repository's GitHub Releases.
Unsigned packages, or packages without platform reputation, can trigger macOS Gatekeeper or Windows SmartScreen warnings.
Such a warning does not prove that the file is malicious, and it does not prove that the file is safe.
Verify the release source and SHA-256 before you continue.

### macOS

1. Double-click LumilioCL in Applications. When it says “LumilioCL” Not Opened, click **Done**. Do not click Move to Trash.
2. Open **System Settings → Privacy & Security** and scroll to the bottom. You will see “LumilioCL” was blocked.
3. Click **Open Anyway** and enter your Mac's login password.
4. Click **Open** in the dialog that follows.

From then on, the application opens normally.

On macOS 14 or earlier: Control-click (or right-click) LumilioCL in Applications, choose **Open**, then click **Open** again.

### The app “is damaged and can't be opened”

This warning can also mean a damaged download or a failed security check.
Download the package again from official Releases and verify its SHA-256 first.
Only if you trust the source and the problem is the quarantine attribute, run this command in Terminal:

```sh
xattr -dr com.apple.quarantine /Applications/LumilioCL.app
```

Then double-click the application again.

### Windows

1. If your browser says the file “isn't commonly downloaded”: in Edge, click **…** next to the download → **Keep** → **Keep anyway**.
2. If a blue “Windows protected your PC” window appears: click **More info**, then **Run anyway**.

### Linux

Warnings depend on your distribution and desktop environment. Install as described above.

## Keep your game data

Removal of the application does not delete games or saves.
The default data directories are:

| System | Directory |
| --- | --- |
| macOS | `~/Library/Application Support/LumilioCL` |
| Windows | `%APPDATA%\LumilioCL` |
| Linux | `~/.local/share/lumilio` |

Back up important saves before you change or remove game data.
