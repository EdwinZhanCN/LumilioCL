# Maintained dependency forks

The directory also carries `gpui-component` 0.7.0, patched through the root
`[patch.crates-io]` so both direct UI imports and GPUI Kit use the same Select.
Its baseline is the crates.io 0.7.0 package, upstream commit
`0c830f4d257e69fdd17200650533ab4ca9a40cc0` (`crates/component`). Imported paths:
`src/`, `locales/`, `tests/`, manifest, build script, README and Apache license.
Only `src/select.rs` is adapted: `SelectState::new_multiple`, committed value
read/write methods and `SelectEvent::Change` support persistent, independently
toggled multi-selection. Single selection retains `Confirm` and closes on
selection. Guards: `crates/lumilio-ui/tests/select_multiple.rs`; run with
`cargo test -p lumilio-ui --test select_multiple`.
The package's original lib tests reference fixtures outside the published crate;
run the dedicated integration target for this local extension.

These are editable source snapshots, owned by LumilioCL (ADR 0029).
Reference-only projects are consulted through their published upstream URLs.
The launcher uses local Cargo paths, not remotely published fork repositories.

| Directory | Upstream | Baseline commit | License |
| --- | --- | --- | --- |
| `cubiomes/` | [Cubitect/cubiomes](https://github.com/Cubitect/cubiomes) | `e61f90580cbdd883214a8054670dacae655e59c0` | [MIT](cubiomes/LICENSE) |
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

- cubiomes: see [the local patch log](cubiomes/LUMILIO.md).

- Nucleation's mesher dependency uses `../schematic-mesher`.
- Schematic-Mesher's development dependency uses `../nucleation`.
- Rendering behaviour follows Minecraft's draw rules (ADR 0034). Each change
  below has a guard that was shown to fail without it; keep them through syncs.

Schematic-Mesher (guards in `schematic-mesher/tests/`, run one with
`cargo test --manifest-path forks/schematic-mesher/Cargo.toml --test NAME`):

- Face culling (`mesher/face_culler.rs`): a full cube with see-through texture
  pixels hides no neighbouring face; leaves do not cull each other; copper
  grates cull only the same block; a model whose one full cube carries overlay
  elements (the grass block) is as solid as a plain cube. Guard: `face_culling`.
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
- Entity facing (`mesher/entity/mod.rs` `facing_rotation_rad`): east and west
  turn the way Minecraft's renderers do (glam's rotation has the opposite sign
  of Minecraft yaw), so chests, double chests, wall banners and mobs face their
  side. Guard: `entity_facing`.
- UV lock (`types/transform.rs` `uvlock_matrix`, `mesher/element.rs`): a
  `uvlock` variant transforms its face UVs like Minecraft's
  `BlockMath.getUVLockTransform`, so rotated stairs, walls, fences and fence
  gates keep world-aligned textures. Guard: `uvlock`.
- Multipart conditions (`resource_pack/blockstate.rs`): `OR`/`AND` nest and a
  leading `!` negates the listed values, as in Minecraft, instead of failing
  the whole blockstate. Guard: `multipart_conditions`.
- Model shading (`resource_pack/model.rs`, `resolver/model_resolver.rs`,
  `mesher/element.rs`): `ambientocclusion: false` turns ambient occlusion off
  and a child that does not set it inherits it; an element's 26.x
  `shade_direction_override` becomes its faces' lighting normal. Guard:
  `model_shading`.
- Fluid surfaces (`mesher/liquid.rs` `corner_height`): corner heights follow
  Minecraft's `calculateAverageHeight`, so a surface slopes toward empty
  neighbors, and waterlogged neighbors count as water. Guard: `fluid_heights`.
- Block tints (`mesher/tint.rs`): follow the color providers Minecraft
  registers — fixed spruce, birch and mangrove leaf colors; no tint for cherry,
  pale oak and azalea leaves; grass color for bushes and flower-bed stems; dry
  foliage for leaf litter; Minecraft's redstone, stem and lily pad colors.
  Guard: `block_tints`.
- Texture aliases (`resource_pack/mod.rs` `texture_aliases`): a missing texture
  name can answer for another path without copying pixels. Nucleation fills it
  (below).
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
- Camera (`src/rendering/camera.rs`, `mod.rs`): optional first-person eye
  position bypasses orbit fitting, with a fixed near plane and reversed depth
  (ADR 0036). The existing RenderConfig continues to produce orbit cameras.
- `UniversalSchematic::undrawable_blocks` (`src/meshing/mod.rs`).
- Pack normalization (`src/meshing/resource_pack_compat.rs` `normalize_zip`):
  keeps only the blockstates, models and textures the mesher's loader reads,
  copies textures without recompressing, and stores rewritten JSON, instead
  of decompressing and deflating a whole client jar on every load. Guard:
  `normalized_jars_keep_only_mesher_assets`.
- Litematic regions (`src/formats/litematic.rs` `parse_regions`): an
  unreadable palette entry becomes air instead of shifting later entries; a
  block index past the palette reads as air, as in Litematica; a truncated
  `BlockStates` array and impossible region bounds are parse errors, not
  panics. Guards: `an_unreadable_palette_entry_keeps_later_entries_in_place`,
  `a_block_index_past_the_palette_reads_as_air`,
  `a_truncated_block_state_array_is_a_parse_error`. `test_parse_regions`
  encoded a stone as palette index 2 of a two-entry palette; its data now uses
  index 1.
- Overlapping regions (`src/meshing/mod.rs` `compute_mesh_output`): regions
  are collected in a fixed order (default, then by name) and where they
  overlap the first block keeps the cell. Guard:
  `overlapping_regions_draw_one_block_per_cell`.
- Entity texture renames (`src/meshing/mod.rs` `with_mesher_texture_aliases`):
  renamed textures are aliased instead of copied, and every old flat
  `entity/<file>` the pack lacks answers for its one moved `entity/<folder>/<file>`
  (26.x banners and the enchanting-table book). Guard:
  `moved_entity_textures_answer_for_their_old_flat_names`.
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
