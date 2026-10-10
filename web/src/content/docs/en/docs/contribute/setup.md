---
title: Set up development
description: Prepare the Rust workspace and the website tools.
---

## Get the source

Install Git, Rustup, and [just](https://just.systems/).
Use the native build tools for your operating system.
On macOS, install the Xcode Command Line Tools.
On Windows, install the Visual Studio C++ build tools.

If you do not have repository write access, create a GitHub fork.
Use your fork URL in the clone command below.

1. Clone the repository.
2. Open its root directory.

```sh
git clone https://github.com/EdwinZhanCN/LumilioCL.git
cd LumilioCL
```

Create a branch for the change:

```sh
git switch -c <change-name>
```

Install the toolchain specified in `rust-toolchain.toml`:

```sh
rustup toolchain install
```

The repository specifies the development toolchain.
Do not replace it with a different Rust version.

## Prepare Linux libraries

The Linux CI job uses these packages:

```sh
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  build-essential clang cmake pkg-config \
  libssl-dev libzstd-dev libfontconfig-dev libfreetype-dev \
  libvulkan-dev libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libx11-dev libx11-xcb-dev libxcb1-dev
```

A desktop session and graphics drivers are necessary to run the launcher.
See the [CI workflow](https://github.com/EdwinZhanCN/LumilioCL/blob/main/.github/workflows/ci.yml) for the current package list.

## Run the launcher

From the repository root, run:

```sh
cargo run -p lumilio-app
```

Use a disposable data directory for manual tests.
Set `LUMILIO_HOME` to that directory before you start the application.
Do not use important game data for a destructive test.

## Prepare the website

Install Node.js 22.12.0 or later.
Use the pnpm version specified by `packageManager` in `web/package.json`.
Run pnpm commands from `web/`.

```sh
cd web
pnpm install --frozen-lockfile
pnpm dev
```

Open the local URL printed by Astro.
Use `/docs/` for Chinese documentation.
Use `/en/docs/` for English documentation.

## Select checks

Run these recipes from the repository root:

| Change | During work | Before handoff |
| --- | --- | --- |
| Rust code | `just test-pkg <crate> [filter]` | `just check` |
| Documentation or harness | `just plans` after plan changes | `just docs` |
| Website or published documentation | `just web` | `just web`; also `just docs` for plan changes |

`just check` runs build, tests, Clippy, and formatting checks in that order.
`just web` runs dependency installation, Astro checks, website tests, and the site build.
Do not report a check as passed until it finishes successfully.
