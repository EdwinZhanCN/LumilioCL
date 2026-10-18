<div align="center">

<img src="assets/icons/linux/hicolor/256x256/apps/app.lumilio.LumilioCL.png" width="128" height="128" alt="LumilioCL icon">

# LumilioCL

A native, fast, extensible Minecraft launcher written in Rust + GPUI, with no Electron or WebView.

[![Latest release](https://img.shields.io/github/v/release/EdwinZhanCN/LumilioCL?include_prereleases&sort=semver)](https://github.com/EdwinZhanCN/LumilioCL/releases/latest)
[![License](https://img.shields.io/badge/license-AGPL--3.0--only-blue)](LICENSE)
[![CI](https://github.com/EdwinZhanCN/LumilioCL/actions/workflows/ci.yml/badge.svg)](https://github.com/EdwinZhanCN/LumilioCL/actions/workflows/ci.yml)

[简体中文](README.md) · English

<img src="assets/screenshots/home.png" width="720" alt="LumilioCL home">

</div>

Game library and modpack import, Modrinth / CurseForge content, managed Java, world maps, crash diagnosis and plugins. See the [website](https://launcher.lumilio.org/en/docs/guide/) for details.

## Download

Available for macOS (Apple silicon), Windows 10 / 11 (x64) and Linux (x64).

- [Website](https://launcher.lumilio.org/en/) or [GitHub Releases](https://github.com/EdwinZhanCN/LumilioCL/releases/latest)
- Installing and SHA-256 checks: [install guide](https://launcher.lumilio.org/en/docs/install/)

## Warnings on first launch?

First verify that the package came from this repository's [GitHub Releases](https://github.com/EdwinZhanCN/LumilioCL/releases/latest). Full instructions are on the [website](https://launcher.lumilio.org/en/docs/install/#warnings-on-first-launch).

- **macOS**: when it says the app was not opened, click **Done**, then click **Open Anyway** at the bottom of **System Settings → Privacy & Security**. On macOS 14 or earlier, Control-click the app and choose **Open**.
- **macOS says the app “is damaged”**: download it again and verify its SHA-256 first. Only if you trust the source, run in Terminal:

  ```sh
  xattr -dr com.apple.quarantine /Applications/LumilioCL.app
  ```

- **Windows**: if Edge says the file “isn't commonly downloaded”, click **…** → **Keep** → **Keep anyway**. If “Windows protected your PC” appears, click **More info** → **Run anyway**.

## Build from source

```sh
cargo run -p lumilio-app   # run
just check                 # build, test, clippy, rustfmt (same as CI)
just package               # package for the current platform into dist/
```

See [Contribute](https://launcher.lumilio.org/en/docs/contribute/) for setup and guidelines, and [`AGENTS.md`](AGENTS.md) for development conventions.

## License and credits

Released under [AGPL-3.0-only](LICENSE). Parts of the code are adapted from [Modrinth App](https://github.com/modrinth/code) (GPL-3.0-only) and [HMCL](https://github.com/HMCL-dev/HMCL) (GPL-3.0-or-later); schematic rendering builds on [Nucleation](https://github.com/Schem-at/Nucleation) (MIT) and [Schematic-Mesher](https://github.com/Schem-at/Schematic-Mesher) (AGPL-3.0-only). Sources are noted in the comments of each adapted file. Font, icon and renderer licenses are in [`ATTRIBUTIONS.md`](ATTRIBUTIONS.md) and [`crates/lumilio-ui/assets/ATTRIBUTIONS.md`](crates/lumilio-ui/assets/ATTRIBUTIONS.md). LumilioCL does not use Modrinth branding.
