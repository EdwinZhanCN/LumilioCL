# Litematica 3D preview: vendored viewer (ADR 0027)

The preview window (`src/model_preview/`) serves these files to a webview over a custom protocol.
Everything is gzip-compressed (`-9n`, so the bytes are reproducible) and decompressed when served.
`viewer.html` is ours; the rest is unmodified upstream output.

| Files | Package | License |
| --- | --- | --- |
| `sr/*` | [`schematic-renderer@1.6.1`](https://github.com/Schem-at/schematic-renderer), `dist/` ES build and the chunks it loads | AGPL-3.0-only (`LICENSE-schematic-renderer.txt`) |
| `three/*` | [`three@0.184.0`](https://github.com/mrdoob/three.js), `build/three.module.js` and `three.core.js` | MIT (`LICENSE-three.txt`) |
| `nucleation/*` | [`nucleation@0.2.18`](https://github.com/Schem-at/Nucleation), the browser build with its WASM | MIT (`LICENSE-nucleation.txt`) |

## What is not here, on purpose

- `dist/pack.zip` from the schematic-renderer package. It is Mojang's default textures. The
  launcher never ships Mojang assets: the preview builds a resource pack from the player's own
  `client.jar` when a window opens.
- The package's UMD build. It expects `three` and `nucleation` as globals; the ES build with an
  import map (see `viewer.html`) is the one that works.
- Chunks the preview never loads (Inspector, exporters, the WebGPU build).

## Updating

1. `npm pack schematic-renderer@<v>`, and the `three` / `nucleation` versions its `package.json`
   asks for (nucleation must stay on `^0.2`; 0.10.x is a different API).
2. Copy the files in the table above, keeping their names (the chunks import each other by name),
   then `gzip -9nc` each one.
3. Open a preview of a real schematic and check the textures, then update the versions here and in
   `assets/ATTRIBUTIONS.md`.
