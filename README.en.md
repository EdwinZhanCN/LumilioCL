<div align="center">

<img src="assets/icons/linux/hicolor/256x256/apps/app.lumilio.LumilioCL.png" width="128" height="128" alt="LumilioCL icon">

# LumilioCL

A native Minecraft launcher written in Rust.

[![CI](https://github.com/EdwinZhanCN/LumilioCL/actions/workflows/ci.yml/badge.svg)](https://github.com/EdwinZhanCN/LumilioCL/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/EdwinZhanCN/LumilioCL?include_prereleases&sort=semver)](https://github.com/EdwinZhanCN/LumilioCL/releases/latest)
[![License](https://img.shields.io/badge/license-AGPL--3.0--only-blue)](LICENSE)
![Platforms](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey)

![Rust](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust)
![GPUI](https://img.shields.io/badge/UI-GPUI-4F46E5)
![gpui-component](https://img.shields.io/badge/gpui--component-0.7-4F46E5)
![Tokio](https://img.shields.io/badge/async-Tokio-463D3B)
![SQLite](https://img.shields.io/badge/storage-SQLite-003B57?logo=sqlite&logoColor=white)
![wgpu](https://img.shields.io/badge/3D_preview-wgpu-2B6CB0)

[简体中文](README.md) · English

</div>

## About

LumilioCL is written in Rust. Its interface is built with [GPUI](https://www.gpui.rs/) and
[gpui-component](https://github.com/longbridge/gpui-component), with no Electron or web view. The
interface is in Simplified Chinese.

- **Library**: create vanilla, Fabric or Quilt games and organize them with favorites, collections
  and search. Import `.mrpack` and MultiMC / Prism modpacks, or bring games over from other
  launchers (MultiMC, Prism, `.minecraft`) without touching the originals.
- **Discover**: browse and install mods, resource packs, shaders and modpacks from Modrinth,
  filtered by game version and loader.
- **Java**: the right Java runtime for each game is downloaded and managed for you.
- **Accounts**: offline accounts (with skins) and third-party auth servers such as LittleSkin.
- **Game page**: manage content, worlds and servers; see screenshots and launch history; make full
  backups and restore them. When a game crashes, the diagnostics tab explains the likely cause.
- **Plugins**: crash analysis, Litematica schematics (material lists and a 3D preview), and
  Discord presence (off by default).

> [!NOTE]
> Microsoft sign-in is built, but Mojang has not yet approved the launcher's app registration.
> Until it does, sign-in fails at the last step and the launcher explains why. Offline and
> third-party accounts work in the meantime.

## Gallery

<div align="center">

<table align="center">
  <tr>
    <td align="center" width="33%"><a href="assets/screenshots/home.png"><img src="assets/screenshots/home.png" width="280" alt="Home"></a></td>
    <td align="center" width="33%"><a href="assets/screenshots/game-library.png"><img src="assets/screenshots/game-library.png" width="280" alt="Library"></a></td>
    <td align="center" width="33%"><a href="assets/screenshots/discovery.png"><img src="assets/screenshots/discovery.png" width="280" alt="Discover"></a></td>
  </tr>
</table>

</div>

## Download

Get the file for your system from
[Releases](https://github.com/EdwinZhanCN/LumilioCL/releases/latest):

| System | File | Requirements |
| --- | --- | --- |
| macOS | `LumilioCL-<version>-macos-arm64.dmg` | Apple silicon (M1 or later), macOS 11 or later; Intel Macs are not supported yet |
| Windows | `LumilioCL-<version>-windows-x64-setup.exe` (installer, recommended)<br>`LumilioCL-<version>-windows-x64-portable.zip` (no install, just unzip) | Windows 10 / 11, 64-bit |
| Linux (Debian, Ubuntu, …) | `LumilioCL-<version>-linux-x64.deb` | 64-bit, Ubuntu 22.04 / Debian 12 or later, a working Vulkan graphics driver |
| Linux (other distributions) | `LumilioCL-<version>-linux-x64.tar.gz` | Same as above |

To install:

- **macOS**: open the `.dmg` and drag LumilioCL into Applications.
- **Windows**: run `setup.exe` and click through; no administrator rights needed. For the portable
  ZIP, unzip it anywhere and run `lumiliocl.exe`.
- **Linux `.deb`**: in the download folder, run `sudo apt install ./LumilioCL-<version>-linux-x64.deb`.
- **Linux `.tar.gz`**: unpack it and run `./install.sh`. It installs for your user only and needs
  no root. To remove it, run `./install.sh --uninstall`.

Uninstalling LumilioCL does not delete your games or saves. They live in
`~/Library/Application Support/LumilioCL` on macOS, `%APPDATA%\LumilioCL` on Windows and
`~/.local/share/lumilio` on Linux.

## A warning when you first open it?

**That is expected, and you only have to deal with it once.**

Apple and Microsoft recognize apps whose developers pay for a yearly signing certificate.
LumilioCL is a free personal project without one, so on first launch macOS and Windows say they
cannot verify the developer. That only means the system does not know who made the app.
**It does not mean anything is wrong with it.** The code is fully public, and GitHub builds the
packages from that public code.

### macOS

1. Double-click LumilioCL in Applications. When it says “LumilioCL” Not Opened, click **Done**.
   Do not click Move to Trash.
2. Open **System Settings → Privacy & Security** and scroll to the bottom. You will see
   “LumilioCL” was blocked. Click **Open Anyway** and enter your Mac's login password.
3. Click **Open** in the dialog that follows.

From then on it opens like any other app.

<details>
<summary>macOS 14 or earlier</summary>

In Applications, Control-click (or right-click) LumilioCL, choose **Open**, then click **Open**
again.

</details>

<details>
<summary>It says the app “is damaged and can't be opened”</summary>

This is another way macOS blocks unsigned apps downloaded from the internet; nothing is
actually damaged. Open Terminal, paste this line and press Return:

```sh
xattr -dr com.apple.quarantine /Applications/LumilioCL.app
```

Then double-click the app again.

</details>

### Windows

1. If your browser says the file “isn't commonly downloaded”: in Edge, click **…** next to the
   download → **Keep** → **Keep anyway**.
2. If a blue “Windows protected your PC” window appears: click **More info**, then **Run anyway**.

### Linux

There is no warning; just install as described above.

### Want to check that the file was not tampered with?

Every release includes `SHA256SUMS.txt`, a fingerprint for each file. Put it in the same folder as
the download and run:

- macOS: `shasum -a 256 -c SHA256SUMS.txt --ignore-missing`
- Linux: `sha256sum -c SHA256SUMS.txt --ignore-missing`
- Windows (PowerShell): `Get-FileHash .\<file> -Algorithm SHA256`, then compare the result with
  the matching line in `SHA256SUMS.txt`.

If it says `OK`, or the two strings are identical, your file is exactly what GitHub built.

## Development

```sh
cargo run -p lumilio-app   # run it
just check                 # build, test, clippy, rustfmt, the same as CI
just package               # package for this platform into dist/
```

- Workspace: `lumilio-core` (launcher logic, independent of the UI), `lumilio-ui` (GPUI views),
  `lumilio-app` (process startup and composition), `lumilio-schematic-render` (native schematic
  rendering), `lumilio-plugin-*` (plugins) and `lumilio-xtask` (release packaging).
- Use `LUMILIO_HOME=/tmp/<any-folder>` for a disposable data folder.
- What the launcher does today is generated from the code into
  [`docs/ia/paths/`](docs/ia/paths/README.md); work in flight is in
  [`.agents/plans/`](.agents/plans/README.md); the look, motion and copy follow
  [`docs/design-language.md`](docs/design-language.md).
- Releases: pushing a `v<version>` tag that matches the workspace version (SemVer) packages all
  three platforms and opens a draft release. See
  [`assets/icons/PACKAGING.md`](assets/icons/PACKAGING.md) and ADR 0032.

<details>
<summary>Review hooks (affect only the current run)</summary>

- `LUMILIO_PAGE` opens a page directly, e.g. `library:2`, `discover:1`, `activity`,
  `instance:1:4:1` (id, section, sub-tab; zero-based).
- `LUMILIO_INSTANCE=<id>` (with `LUMILIO_INSTANCE_TAB`, `LUMILIO_INSTANCE_GROUP`) and
  `LUMILIO_PROJECT=mod/sodium` open a game or a project detail.
- `LUMILIO_REVIEW_APPEARANCE=light|dark`, `LUMILIO_REVIEW_MIN=1` (720×480),
  `LUMILIO_REVIEW_STILL=1` (reduced motion).

</details>

## License

LumilioCL is licensed under the GNU Affero General Public License, version 3 only
(`AGPL-3.0-only`); see [`LICENSE`](LICENSE). Code adapted from upstream names its source and
license in a comment on the file or function. Bundled fonts and icons are listed in
[`crates/lumilio-ui/assets/ATTRIBUTIONS.md`](crates/lumilio-ui/assets/ATTRIBUTIONS.md), and the
native rendering dependencies in [`ATTRIBUTIONS.md`](ATTRIBUTIONS.md).

## Acknowledgements

- **[Modrinth App](https://github.com/modrinth/code)**: its Rust launcher backend, `app-lib`, is
  the first place LumilioCL looks for how authentication, Java runtimes, instances, modpacks and
  processes are built. Some code is adapted from it (GPL-3.0-only). Modrinth's branding is not
  used.
- **[HMCL](https://github.com/HMCL-dev/HMCL)**: LumilioCL's main reference for feature breadth,
  platform differences and edge cases. Some code is adapted from it (GPL-3.0-or-later).
- **[Nucleation](https://github.com/Schem-at/Nucleation)** (Schem-at, MIT): Litematica parsing,
  meshing and GPU rendering.
- **[Schematic-Mesher](https://github.com/Schem-at/Schematic-Mesher)** (Schem-at, AGPL-3.0-only):
  block geometry and meshing for liquids, chests and more.

The last two are maintained as editable source snapshots in [`forks/`](forks/README.md), which
records their baselines and local changes. Thank you to everyone who built and contributes to
these projects.
