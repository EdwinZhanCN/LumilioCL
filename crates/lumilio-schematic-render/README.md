# Native schematic renderer

`Scene::load` parses and meshes a `.litematic` with a `BakedPack` from
`lumilio-block-assets` (the host bakes or downloads it) and the same game
version's entity layers (`EntityLayers`, from `entities.dat.z`), which draw
block entities and mobs through the mesher's binding table;
`Scene::render` draws an orbit camera into BGRA pixels. Both calls block, and
the scene stays on its owning worker thread. There is no launcher or GPUI
dependency. The host supplies game assets and the clear colour.

When the pack was baked from a client JAR, `Scene::load` first converts the
schematic to the pack's data version, so renamed blocks find their models.
`Scene::undrawable_blocks` lists the block IDs the pack still cannot draw; the
host should tell the user (ADR 0034).

The schematic's entities are not part of the block mesh: each distinct look is
meshed once and every entity is an instance of it at its exact position and
yaw. The renderer draws a model's instances in one call and skips those outside
the view, and in first-person view those past `ENTITY_DISTANCE` blocks.
`Scene::undrawable_entities` lists entity IDs nothing draws (no binding row, or
no layer in this game version); the host shows them like undrawable blocks.

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
Set `LUMILIO_SCHEMATIC` and `LUMILIO_PACK` (a client jar, resource-pack zip or
folder, baked before loading) to measure real, local inputs; `LUMILIO_ENTITIES`
(an `entities.dat.z`) adds the pack's game version's entity layers; `LUMILIO_FRAME_PATH`
saves the first 1800×1200 frame as PNG for before/after pixel comparisons.

With the baked bundle in `target/block-assets/` (`cargo xtask bake-blocks`),
`five_hundred_armor_stands_frame_time` measures 500 armor stands against the
same build without them (plan schematic-studio T8), and `entity_review_frames`
saves every bound mob, and `LUMILIO_SCHEMATIC` when set, under
`LUMILIO_FRAME_DIR` for visual review. Both are ignored by default; run them
like `preview_timings`.
