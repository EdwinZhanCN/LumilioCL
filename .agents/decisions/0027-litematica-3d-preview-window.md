# 0027 — Litematica 3D preview: a separate webview window running schematic-renderer

- Status: accepted
- Date: 2026-10-04

## Context

The Litematica plugin lists schematics and their material lists, but players also want to see the
build. A native 3D renderer would be a project of its own. `gpui-wry` embeds a webview in GPUI, and
`schematic-renderer` is an existing Three.js + Rust/WASM viewer that reads `.litematic`.

Verified on macOS (the plan's V1–V6): the renderer works offline in WKWebView, a resource pack built
from a vanilla `client.jar` gives correct textures for 1.21.11 and 26.2, and schematics up to 27
million blocks of volume render at 60 fps in 7 seconds or less. `gpui-wry` draws a native view
above the GPUI window, so anything GPUI paints over it (dialogs, toasts, the floating navigation, a
scrolling page) is hidden, and it supports macOS and Windows only.

## Decision

- The preview opens in its own window, with the webview filling it. It is never embedded in the
  scrolling instance page. The page shows a card with a 3D preview key.
- A plugin only asks for it: `View::Model { file }` in the view tree, where `file` lies under the
  plugin's `ReadGameFiles` grant. The host reads the file through the same checks as `read_file`.
  `lumilio-plugin-api` knows nothing about webviews, and the webview exists only in `lumilio-ui`.
- The viewer is schematic-renderer 1.6.1 with three.js 0.184.0 and nucleation 0.2.18, vendored
  unmodified and gzip-compressed in `crates/lumilio-ui/assets/litematic-viewer/` (about 5.6 MB). It
  is the ES build behind an import map: the UMD build needs globals nobody provides. All three
  licenses ship with the files. This project is AGPL-3.0-only, which is compatible.
- Textures come from the player's own game. The host builds a resource pack from the instance's
  `client.jar` (only `pack.mcmeta` and `assets/minecraft/{blockstates,models,textures,atlases,
  items}`) when the viewer asks for it. Mojang assets are never vendored; the package's own
  `pack.zip` is left out. With no jar (game not installed) the preview opens with a note and
  without textures.
- The renderer stores packs in the webview's IndexedDB and refuses to work without it, so the
  webview keeps persistent storage. The pack's name carries the game version and the jar's identity,
  and the viewer removes packs stored under other names.
- The plugin prepares what the viewer is given: `InstanceTab::model` returns the file as it is by
  default, and an error from it fails that preview, not the plugin. Litematica merges the regions
  of a schematic into one first, because the renderer mixes up the per-region palettes of a
  schematic that has several (glass showed as the soul sand with the same index in another
  region). The host sets a size limit on what comes back.
- The viewer frames the schematic again after it is rendered: the renderer's own framing on load
  sometimes ran before the meshes were ready, and the camera ended up inside a large build.
- Platforms: macOS and Windows. Linux has no preview (the key is hidden there). Windows was not
  tested.

## Consequences

- Positive: a real, textured, rotatable preview for a small amount of our own code, offline, with no
  Mojang assets in the repository or the installer.
- Negative / trade-offs: the preview is not inline; about 25 MB (6 MB compressed) of third-party
  JavaScript and WASM ships and must be updated by hand; a block renamed between game versions
  (`chain` is `iron_chain` now) is missing when the schematic is older than the instance; no Linux.
- Known limits of the renderer (1.6.1) that we have not worked around: water and lava are drawn
  as blocks of the level's height without slopes; a double chest is two single chests; soul fire is
  a cube; signs, shulker boxes, player heads and redstone wire are missing or wrong; a door lies
  flat. The renderer supports only single chest, trapped chest and ender chest models.
- Not decided: oldest game version supported (nothing before 1.21.11 was tried), a custom bundle
  that drops what the preview never loads.

## What shipped

- `lumilio-core`: `model_assets` (resource pack from a `client.jar`), `PluginHost::read_model`
  (a file under the plugin's `ReadGameFiles` grant, plugin on), and
  `LauncherService::plugin_model` (schematic bytes plus the pack, or none when the game is not
  installed).
- `lumilio-plugin-api`: `View::Model { file }`. The Litematica detail shows it first.
- `lumilio-ui`: the `model_preview` module (the vendored viewer served from memory over a custom
  protocol, and the window on macOS and Windows), and a card with an open key in the plugin view
  renderer (no key where unsupported).
- Verified on macOS: the viewer in a real GPUI window with textures from a 26.2 `client.jar`;
  1.21.11 and 26.2 packs; schematics up to 27 million blocks of volume.
- Not verified: Windows; the whole click path from the instance page by a person; versions before
  1.21.11.
