# Maintained native rendering forks

These are editable source snapshots, owned by LumilioCL (ADR 0029).
They are separate from the read-only reference repositories in `3rd-party/`.
The launcher uses local Cargo paths, not remotely published fork repositories.

| Directory | Upstream | Baseline commit | License |
| --- | --- | --- | --- |
| `nucleation/` | [Schem-at/Nucleation](https://github.com/Schem-at/Nucleation) | `51de345adce496c34e73a6458b2a2d107a1148c2` | [MIT](nucleation/LICENSE) |
| `schematic-mesher/` | [Schem-at/Schematic-Mesher](https://github.com/Schem-at/Schematic-Mesher) | `286323e472cd8a5363b66a035a3be02ecdd4f164` | [AGPL-3.0-only](schematic-mesher/LICENSE) |

Nucleation includes its root manifest, build script, notices, README/release
notes, `src/`, `crates/`, `data/blockpedia/`, `tools/mc-data/`, `examples/`,
`tests/` and `benches/`. Its optional sibling crates remain present so the
upstream manifests and test references stay meaningful. Schematic-Mesher
includes its manifest, notices, README, `src/` and `tests/`. Generated language
bindings, website builds, Git metadata and unrelated packaging are omitted.
No client JAR or launcher runtime resource pack is imported. Preview textures
continue to come from the user's installed game.

The fork packages are excluded from the launcher's workspace membership. They
are still compiled as dependencies and covered by the adapter's GPU tests;
their entire optional tool/test suite is not part of every launcher check.
Keep upstream license headers and API attribution when changing source.

## Local changes

- Nucleation's mesher dependency uses `../schematic-mesher`.
- Schematic-Mesher's development dependency uses `../nucleation`.
- Rendering behaviour follows Minecraft's draw rules (ADR 0034). Each change
  below has a guard that was shown to fail without it; keep them through syncs.

Schematic-Mesher (guards in `schematic-mesher/tests/`, run one with
`cargo test --manifest-path forks/schematic-mesher/Cargo.toml --test NAME`):

- Face culling (`mesher/face_culler.rs`): a full cube with see-through texture
  pixels hides no neighbouring face; leaves do not cull each other; copper
  grates cull only the same block. Guard: `face_culling`.
- Element rotation (`types/transform.rs`, `mesher/element.rs`): the 26.x Euler
  form `{x, y, z}` parses, both forms use Minecraft 26.3 `CuboidRotation`
  semantics, and normals turn with the element. Guard: `element_rotation`.
- Signs and beds (`mesher/entity/mod.rs`, `mesher/element.rs`): when the block
  model has elements (26.2+), the legacy entity is not drawn. Guard:
  `block_model_entities`.
- Coplanar faces (`mesher/element.rs` `separate_coplanar_faces`): overlapping
  faces of one block in one plane and facing move out 1/1024 block per earlier
  face. Guard: `coplanar_faces`.
- `undrawable_blocks` (`mesher/mod.rs`, re-exported): names blocks no model or
  stand-in geometry can draw. Guard: `undrawable_blocks`.
- The crate's own unit tests and `tests/atlas_uv_regression.rs` do not compile
  at the baseline: upstream `253d2b6` changed `MesherOutput` layers to
  `MeshLayer` without updating them. Local guards therefore live in new
  integration test files that share `tests/common/`.

Nucleation (unit guards run with the `--lib` command under Synchronizing
upstream; its lib tests also write `simple_cube.litematic`,
`test_schematic.schem` and `tests/output/` into the fork, delete them after):

- Renderer (`src/rendering/gpu.rs`, `shader.wgsl`, `camera.rs`): back-face
  culling on all layers; reversed depth (`compute_view_proj_reversed`,
  `GreaterEqual`, clear 0); opaque and cutout without blending (`fs_solid`);
  translucent triangles re-sorted back to front each frame (`TriangleOrder`).
  Guards: `reversed_depth_keeps_framing_and_flips_depth_order`,
  `translucent_triangles_draw_farthest_first`.
- `UniversalSchematic::undrawable_blocks` (`src/meshing/mod.rs`).
- `src/meshing/item_model.rs` exports only single-axis element rotations; an
  Euler rotation is omitted there.

## Synchronizing upstream

1. Clone each upstream into a temporary directory and check out the baseline
   above. Fetch the proposed new revision and inspect `git diff BASE NEW` for
   the imported paths. Do not replace the local tree wholesale.
2. Apply the relevant upstream source changes to the corresponding fork,
   preserving our local path dependencies and local bug fixes. For a change
   adapted from a specific upstream file, retain its source/license notice.
   Resolve conflicts explicitly; review model, shader and data changes together.
3. Run `just test-pkg lumilio-schematic-render`. For an upstream regression
   test, run that specific test using the fork's `--manifest-path`, `-p` package,
   target and required features rather than testing its whole workspace or
   enabling every optional feature. For example, a native renderer unit guard
   can run with `cargo test --manifest-path forks/nucleation/Cargo.toml -p
   nucleation --no-default-features --features meshing,rendering --lib FILTER`.
4. Run `just check`, inspect real rendering for visual changes, then update the
   baseline above and [attributions](../ATTRIBUTIONS.md) only if the imported
   paths are fully synchronized to that revision. Otherwise record the specific
   backported commit under Local changes and retain the baseline.

Record local fixes under Local changes with their regression tests. The enclosing
LumilioCL Git history tracks those edits; do not add nested `.git` directories.
