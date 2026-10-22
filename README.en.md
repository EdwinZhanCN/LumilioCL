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

## Disclaimer

1. **Not an official product**: **LumilioCL** is an independent, third-party open-source project. It is **not** an official Minecraft product, and it is not approved by, endorsed by or affiliated with Mojang Studios, Microsoft or NetEase.
2. **No hosting, tool only**: LumilioCL is only a local client tool. It manages local files, configures JVM arguments and starts the game. LumilioCL does **not store, host or directly distribute** Minecraft game files, official resources or other copyrighted commercial assets. The launcher downloads all core game files on the user's computer, directly from Mojang's official services or from a public third-party mirror that the user selects.
3. **Genuine accounts and lawful use**: Please support the official game. To download or play the game with this launcher, you need a legitimate Minecraft account, and you must obey the [Minecraft End User License Agreement (EULA)](https://www.minecraft.net/eula). The developers are not responsible for any attempt to bypass account verification, modify game files or break the official terms.
4. **Third-party resources and services**: This software can use third-party APIs and mirror services, such as BMCLAPI, CurseForge and Modrinth. These services and their content (including, but not limited to, mods, modpacks, resource packs and shaders) belong to their platforms and original authors, who are responsible for them. LumilioCL gives no warranty for the availability or security of third-party services, or for the legality of their content.
5. **No warranty**: This software is provided "AS IS", without warranty of any kind, express or implied. In no event are the project contributors liable for any data loss, account problems, or direct or indirect damages that come from the use of, or the inability to use, this software.
