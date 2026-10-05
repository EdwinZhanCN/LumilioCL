# 0028 — Litematica 3D preview: Nucleation and wgpu draw it in the page

- Status: accepted
- Date: 2026-10-04

Dependency ownership is superseded by ADR 0029: the same upstream baselines
are now maintained as local Rust source forks. The native preview architecture
remains unchanged.

## Context

ADR 0027 put the preview in a separate webview window running schematic-renderer. Using it showed
the limits: the renderer mixed the palettes of schematics with several regions, and drew water,
double chests, doors, signs and fire wrongly. `gpui-wry` paints a native view above the window
(dialogs, toasts and a scrolling page are hidden behind it) and exists for macOS and Windows only.
schemat.io, the site that shows the same files well, uses Nucleation (MIT) for parsing and
Schematic-Mesher (AGPL-3.0-only) for meshing, both in Rust. Nucleation's `rendering` feature
draws with wgpu without a window.

Initial macOS trials established the offscreen-to-GPUI path and stable image-cache memory over
3000 frames (`Window::drop_image` on each replaced frame). Integration's GPU pixel test then
showed that the pinned GPU renderer ignores greedy material layers, so earlier greedy-mode
timings do not establish complete rendering. With the corrected non-greedy path, the maintainer's
real schematic and Fabric 26.2 jar measured 204 ms for input reading, pack loading, parsing,
meshing and GPU setup combined; 2.78 ms per 1800x1200 frame, including 0.73 ms for BGRA conversion
(release, macOS Apple Silicon; local warm files, 100-frame average).

## Decision

- The preview is drawn natively and sits inside the instance page. There is no webview, no
  JavaScript and no separate window, and every platform runs the same code.
- A new crate, `lumilio-schematic-render`, depends only on Nucleation and `pollster`. It parses a
  `.litematic`, meshes it with the pack's textures and draws one frame for an orbit camera, as BGRA
  pixels (the order GPUI wants). It knows nothing about GPUI or the launcher.
- Nucleation is a git dependency at rev `51de345` with `meshing` and `rendering` only. The
  crates.io 0.10.24 does not build: it names `schematic-mesher = "0.2.0"` and loses the git half
  on publish, which resolves to an older mesher. The mesher is pinned by Nucleation to `286323e`.
- Textures stay the player's own, built from `client.jar` as in ADR 0027 (`model_assets`). Mojang
  assets are never vendored. Without an installed game there is no pack and no preview.
- `lumilio-ui` owns a worker thread that holds the scene (the renderer is not `Send`). The view
  sends the camera and size; requests coalesce and the newest one wins. It draws only when the
  camera or the size changes, at physical pixels, and drops the previous frame's image.
- The plugin side keeps what ADR 0027 decided: `View::Model { file }`, and the host reads the
  file through the `ReadGameFiles` checks. The `model` hook and the region merge that existed to
  suit schematic-renderer are removed; Nucleation handles several regions itself.
- Licenses: Nucleation is MIT; Schematic-Mesher is AGPL-3.0-only, the same as this project (ADR
  0025). Their notices and pinned revs are recorded in the new crate's `Cargo.toml` and the
  README of the crate and `ATTRIBUTIONS.md`.
- With no usable graphics adapter the view shows one sentence and the rest of the page keeps working.
- Native meshing keeps Nucleation's default non-greedy mode. At the pinned revision the GPU
  renderer does not consume `MeshOutput::greedy_materials`; enabling it drops solid cubes.
  The red-cube GPU pixel test detects this. Earlier greedy-mode timing observations are not
  evidence that the complete scene renders; native performance is measured again without it.

## Consequences

- Positive: correct blocks (fluids, double chests, glass) from an engine that is maintained for
  this; the preview is part of the page, so dialogs, toasts and scrolling work; no browser engine
  or 25 MB of JavaScript and WASM; Linux is no longer left out in principle.
- Negative / trade-offs: wgpu and Nucleation make a cold build much longer (51 s release for the
  crate alone; the first debug build of the example took about 21 minutes) and add git
  dependencies, so CI must reach GitHub; a resize rebuilds the renderer's targets; each frame is
  copied from the GPU and uploaded again until GPUI can take a GPU texture, and only that last
  step would change then.
- Known limits: Minecraft 26.2 resources lack the bed and sign textures and have one hanging-sign
  model Nucleation cannot read (1.21.11 is fine). Windows and Linux are not tested. Animated
  blocks are drawn still.

## What shipped

The instance's schematic detail now embeds the native preview with worker-owned
GPU rendering, coalesced camera requests, physical-pixel frames, drag/wheel and
keyboard controls, reset, retry and plain error feedback. The old webview code,
assets and dependencies are removed; game assets still come from the installed
instance. GPU pixel and GPUI interaction tests and `just check` passed.

On 2026-10-05 the maintainer accepted the remaining native visual and interaction
checks and deferred glass flicker during orbit and Minecraft 26.2 resource
compatibility to backlog. Those issues are not claimed fixed. The implementation
plan is closed; Windows/Linux compatibility remains unverified locally.
