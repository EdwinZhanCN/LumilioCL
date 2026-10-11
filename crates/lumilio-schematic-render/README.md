# Native schematic renderer

`Scene::load` parses and meshes a `.litematic` with a resource-pack zip;
`Scene::render` draws an orbit camera into BGRA pixels. Both calls block, and
the scene stays on its owning worker thread. There is no launcher or GPUI
dependency. The host supplies game assets and the clear colour.

When the pack is a client JAR, `Scene::load` first converts the schematic to the
data version in its `version.json`, so renamed blocks find their models.
`Scene::undrawable_blocks` lists the block IDs the pack still cannot draw; the
host should tell the user (ADR 0034).

Greedy meshing stays off: the pinned native GPU renderer does not draw the
mesher's separate greedy material layers. A GPU pixel test protects solid cubes.

Nucleation and Schematic-Mesher are locally maintained under `forks/`, based on
`51de345` (MIT) and `286323e` (AGPL-3.0-only). See
[fork maintenance](../../forks/README.md), [attributions](../../ATTRIBUTIONS.md)
and ADRs 0028/0029/0034 for the dependency choice, draw rules and limitations. The adapter follows the API
usage in Nucleation's `examples/readme/meshing-and-rendering/rust/src/main.rs`.

Run `just test-pkg lumilio-schematic-render` for camera, colour conversion,
error, data-version and GPU pixel tests. The pixel test explains its skip if no adapter is
available. For release measurements, run:

```sh
cargo nextest run --release -p lumilio-schematic-render preview_timings --ignored --no-capture
```

By default this measures a generated stone cube and a generated minimal pack.
Set `LUMILIO_SCHEMATIC` and `LUMILIO_PACK` to measure real, local inputs.
