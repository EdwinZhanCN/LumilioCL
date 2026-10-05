# Native schematic renderer

`Scene::load` parses and meshes a `.litematic` with a resource-pack zip;
`Scene::render` draws an orbit camera into BGRA pixels. Both calls block, and
the scene stays on its owning worker thread. There is no launcher or GPUI
dependency. The host supplies game assets and the clear colour.

Greedy meshing stays off: the pinned native GPU renderer does not draw the
mesher's separate greedy material layers. A GPU pixel test protects solid cubes.

Nucleation and Schematic-Mesher are locally maintained under `forks/`, based on
`51de345` (MIT) and `286323e` (AGPL-3.0-only). See
[fork maintenance](../../forks/README.md), [attributions](../../ATTRIBUTIONS.md)
and ADRs 0028/0029 for the dependency choice and limitations. The adapter follows the API
usage in Nucleation's `examples/readme/meshing-and-rendering/rust/src/main.rs`.

Run `just test-pkg lumilio-schematic-render` for camera, colour conversion,
error and GPU pixel tests. The pixel test explains its skip if no adapter is
available. For release measurements, run:

```sh
cargo test --release -p lumilio-schematic-render preview_timings -- --ignored --nocapture
```

By default this measures a generated stone cube and a generated minimal pack.
Set `LUMILIO_SCHEMATIC` and `LUMILIO_PACK` to measure real, local inputs.
