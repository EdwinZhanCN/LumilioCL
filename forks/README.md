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
includes its manifest, notices, README, `src/` and `tests/`, plus the local
`data/` tables. Generated language
bindings, website builds, Git metadata and unrelated packaging are omitted.
No client JAR or launcher runtime resource pack is imported. The forks read
block assets only as a `BakedPack` from `crates/lumilio-block-assets` (plan
schematic-studio, D1), which bakes them from the user's game.

The fork packages are excluded from the launcher's workspace membership. They
are still compiled as dependencies and covered by the adapter's GPU tests;
their entire optional tool/test suite is not part of every launcher check.
Keep upstream license headers and API attribution when changing source.

## Local changes

- cubiomes: see [the local patch log](cubiomes/LUMILIO.md).

- Nucleation's mesher dependency uses `../schematic-mesher`.
- Schematic-Mesher's development dependency uses `../nucleation`.
- Both depend on `../../crates/lumilio-block-assets` (Nucleation through its
  `meshing` feature). Schematic-Mesher's manifest has an empty `[workspace]`
  so its guards also run from a Git worktree nested in another checkout.
- Both use `zip` 9 (upstream: 2). crates.io yanked every older release on
  2026-10-16, after the zip 9.0 extraction advisories, and the forks' lockfiles
  are not tracked, so `just forks` resolves afresh. zip 9 entry names are
  fallible: a name scan treats an undecodable name as no match, and Nucleation's
  world loader and Schematic-Mesher's pack loader return the error.
  Nucleation's `ffmpeg-sidecar` development dependency leaves out its default
  `download_ffmpeg` feature, which requires zip 4; `render_schematic` runs an
  installed ffmpeg.
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
- Signs and beds (`mesher/element.rs`, the binding rows' `model_replaces`):
  when the block model has elements (26.2+), the legacy entity is not drawn.
  Guard: `block_model_entities`.
- Coplanar faces (`mesher/element.rs` `separate_coplanar_faces`): overlapping
  faces of one block in one plane and facing move out 1/1024 block per earlier
  face. Guard: `coplanar_faces`.
- `undrawable_blocks` (`mesher/mod.rs`, re-exported): names blocks no model or
  stand-in geometry can draw. Guard: `undrawable_blocks`.
- Missing-texture placeholder (`mesher/element.rs` `placeholder_models`, plan
  schematic-studio D24): a block whose state or model cannot be resolved is
  meshed as one full cube on the missing-texture tile instead of being
  skipped, so the shape stays whole and the block can be picked. Guard:
  `undrawable_blocks` (`an_undrawable_block_is_drawn_as_a_missing_texture_cube`).
- Entity facing (`mesher/entity/mod.rs` `facing_rotation_rad`): east and west
  turn the way Minecraft's renderers do (glam's rotation has the opposite sign
  of Minecraft yaw), so chests, double chests, wall banners and mobs face their
  side. Guard: `entity_facing`.
- Entity geometry from the game (`mesher/entity/`, `data/entity_bindings.json`,
  plan schematic-studio T7, D5): `detect_block_entity`, `detect_mob` and the
  hand-copied model files (`chest.rs`, `bed.rs`, `sign.rs`, `mob.rs`,
  `armor_stand.rs`, `player.rs` and 22 more, each a copy of one game version)
  are deleted. The caller hands the version's model layers, baked from the
  game's `LayerDefinitions` into `entities.dat.z`, to
  `ResourcePack::with_entity_layers`. A binding row names an id pattern
  (`block:{color}_bed`, `entity:{wood}_boat`), its layers, texture candidates,
  the renderer's placement (`transforms`, `facing`, `renderer_scale` for
  versions whose layers do not carry the scale yet), a static pose (book
  open, boat paddles at rest, hidden faces), conditional poses, overlays
  (saddles, wool, horse armor), armor layers per slot with the older shared
  armor layer as fallback, and the baby layer or the adult scaled down.
  `bindings.rs` reads the table, `layers.rs` turns a layer into parts,
  `model.rs` (the box unwrap and face order of the original `mod.rs`) emits
  faces, `render.rs` places them. Composition stays code switched on by a
  row's `compose`: banner patterns (`banner.rs`), sign text (`sign_text.rs`),
  pot sherds (`decorated_pot.rs`), skins (`skin.rs`, from `skull.rs` and
  `player.rs`), villager clothes, and item frames, now drawn from the pack's
  own `item_frame` block model (`item_frame.rs`) with their item
  (`item_render.rs`). A version without a layer draws nothing for that row,
  and `undrawable_blocks` reports a bound entity the pack has no layer for.
  Guards: `entity_layers` (faces, overlays, armor fallback, baby fallback,
  pose properties, missing layers), `entity_bindings` (rows pick the right
  layer and texture), `entity_binding_lint` (every row's layers exist in the
  26.3 dump, or 26.1.2 for beds and signs, every other layer in some version,
  and a texture of every row in that version's baked pack; reads
  `target/block-assets/` or `LUMILIO_BLOCK_BUNDLE`/`LUMILIO_ENTITIES`, says
  when absent, and its own test shows a misspelt layer, texture or pattern is
  reported), `entity_facing`, `block_model_entities`, `uvlock`, and the
  regression bridge `baked_geometry_matches_the_hand_copied_models` in
  `lumilio-block-assets`. The guards read `tests/fixtures/entity_layers.json`,
  layers picked from the real dump, so they need no `target/`.
  `Facing::turns_with_yaw` (`bindings.rs`, plan schematic-studio T8) says
  which rows Nucleation may mesh once facing south and turn per instance.
- Entities hung on a block (plan schematic-studio T33, D32). Schematic-Mesher
  `item_frame.rs` `hanging_pose`: the steps Minecraft's item frame renderer
  takes, the game's frame model being on the cell's +z side; the original
  turned it to the opposite side of its cell. `item_render.rs`
  `build_item_transform`: from that pose, 7/16 toward the wall (1/2 when the
  frame is invisible), `ItemRotation` eighths clockwise, halved, then the
  `fixed` display transform turned X, Y, Z as the game composes it; flat
  items one pixel thick about the centre (7.5/16 to 8.5/16, not 7/16 to
  8/16). `element.rs` `add_item_frame` leaves the frame out of an invisible
  one. Nucleation `entity_instances` `hung_facing`: an entity that saves its
  block (`block_pos` or `TileX`) is hung, facing its six-way `Facing` or
  four-way `facing`, drawn in the block it hangs in; `entity_to_input_block`
  carries `ItemRotation` and `Invisible`. Guards: `lumilio-schematic-render`
  `tests/hanging.rs` (six facings against the wall, the item on its frame,
  clockwise and not mirrored), Nucleation
  `a_hung_entity_faces_as_saved_and_others_are_not_hung`.
- Entities drawn from their renderer looks (plan schematic-studio T34, D32).
  Schematic-Mesher `ResourcePack::entity_looks` keeps what each entity type's
  own renderer used in that game version (`lumilio_block_assets::EntityLook`,
  dumped by `tools/entity-dump`); `bindings.rs` `find_for`/`known_for` fall
  back, after the table, on a row made from a living entity's look (its main
  layer and default texture, the `mob` transform, turning with its yaw, its
  baby layer and texture), made once per distinct look. The table's rows come
  first. `element.rs`, `mesher/mod.rs` `undrawable_blocks` and Nucleation
  `entity_instances::placed` use them. Guards: `lumilio-schematic-render`
  `tests/looks.rs` (a bee without a row drawn and turned, its baby smaller,
  reported without its look, the zombie's row winning over a look).
- Sliceable meshes (plan schematic-studio T25). Schematic-Mesher
  `MesherConfig::keep_faces_along` and `FaceCuller::keeping_faces_along`:
  faces between neighbours along that axis are never culled and no block is
  skipped as enclosed, so a slice clipped at any layer shows that layer's
  top faces. Nucleation `MeshConfig::keep_faces_along` carries it and
  `meshing` re-exports `Axis`. Guard: `lumilio-schematic-render`
  `a_slice_shows_its_layer_from_above_and_clicks_land_on_it`.
- Every entity type drawn (plan schematic-studio T35, D32). Schematic-Mesher
  `bindings.rs`: rows by renderer class (`renderers` in
  `data/entity_bindings.json`, `by_renderer`, `{look_texture}` the look's own
  texture) between the id rows and the rows made from looks; `aim` (drawn at
  the exact position, turned by yaw and pitch with the renderer's signs, from
  an origin), `carries` (`CarriedBlock`: a block state meshed as the
  schematic's blocks are, then moved by the row's facing and steps and a
  lifted offset: minecart contents, falling blocks, primed TNT, block
  displays; `element.rs` `add_carried_block`, `parse_state`), `sprite`
  (`Sprite`, `add_sprite`, the experience orb's icon by value), `billboard`,
  `tint` for the main layers (`dye:<variable>`, `wool` for sheep), and
  `nothing` (lightning, evoker fangs, fishing bobbers: the game draws none
  from a save); `draws` says whether a row draws anything. Composition:
  `Painting` (`painting.rs`, sized by `ResourcePack::paintings`), `ItemDisplay`
  (`item_render.rs` `render_item_in_context`), `TextDisplay`
  (`sign_text.rs` `composite_text_display`, `add_text_display`, both sides).
  Dye tints are `DyeColor`'s diffuse colours (`entity/mod.rs` `dye_diffuse`,
  `wool_rgb`; banners used map colours); default skins follow
  `DefaultPlayerSkin.get` by UUID (`skin.rs`). Six id rows a look draws alike
  in every version are gone. Guards: `tests/entity_coverage.rs` (every type of
  every bundled version; `cargo xtask bake-blocks` runs the same check),
  `entity_binding_lint` (renderer rows, and rows a look makes redundant),
  `lumilio-schematic-render` `tests/coverage.rs`, `skin.rs`
  `a_uuid_picks_the_game_s_default_skin`. The export tests now build
  `MeshLayer`s, and `atlas_uv_regression` passes without upstream's
  `pack.zip`, so `just forks` runs the whole suite.
- UV lock (`types/transform.rs` `uvlock_matrix`, `mesher/element.rs`): a
  `uvlock` variant transforms its face UVs like Minecraft's
  `BlockMath.getUVLockTransform`, so rotated stairs, walls, fences and fence
  gates keep world-aligned textures. Guard: `uvlock`.
- Multipart conditions (`resource_pack/blockstate.rs`): `OR`/`AND` nest and a
  leading `!` negates the listed values, as in Minecraft, instead of failing
  the whole blockstate. Guard: `multipart_conditions`.
- Baked packs (`resource_pack/baked.rs` `ResourcePack::from_baked`): reads a
  `lumilio_block_assets::BakedPack` — models already flattened, texture
  variables already texture ids, item models with their display transforms,
  colormap samples — into the mesher's types without knowing any game
  version. The guards above build their fixtures as pack files baked by
  `lumilio-block-assets` (`tests/common`). Guard: `baked_spellings` (a pack in
  26.3 spelling meshes with no load-time rewriting).
- Renamed textures (`resource_pack/mod.rs` `get_texture`): the entity code
  asks for the newest names the baker stores (`entity/banner/banner_base`,
  `entity/minecart/minecart`, `entity/cat/cat_tabby` …); an older name built
  at run time is read through `lumilio_block_assets::canonical_texture`, the
  baker's one rename table (`data/texture_renames.json`). Guards:
  `tint_table` `entity_texture_names_resolve` (every entity texture name left
  in the mesher's source exists in the newest baked version; the binding
  table's are checked by `entity_binding_lint`) and Nucleation's
  `a_baked_pack_answers_for_renamed_entity_textures`.
- Model shading (`resource_pack/model.rs`, `resolver/model_resolver.rs`,
  `mesher/element.rs`): `ambientocclusion: false` turns ambient occlusion off
  and a child that does not set it inherits it; an element's 26.x
  `shade_direction_override` becomes its faces' lighting normal. Guard:
  `model_shading`.
- Fluid surfaces (`mesher/liquid.rs` `corner_height`): corner heights follow
  Minecraft's `calculateAverageHeight`, so a surface slopes toward empty
  neighbors, and waterlogged neighbors count as water. Guard: `fluid_heights`.
- Block tints (`mesher/tint.rs`, `data/tints.json`): a table of block ids to
  tint kinds following the color providers Minecraft registers — fixed
  spruce, birch and mangrove leaf colors; no tint for cherry, pale oak and
  azalea leaves; grass color for bushes and flower-bed stems; dry foliage for
  leaf litter; Minecraft's redstone, stem and lily pad colors. `tint.rs` only
  looks colors up; the plains grass, foliage and dry-foliage colors come from
  the baked pack's colormap samples (`TintProvider::sample_colormaps`).
  Guards: `block_tints`; `tint_table` (every table id is a block of the newest
  baked version, and the samples are the game's plains colors; it reads
  `target/block-assets/blocks.dat.z` or `LUMILIO_BLOCK_BUNDLE` and says when
  the bundle is absent).
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
- One mesh at a time (`src/rendering/gpu.rs` `set_only_mesh`,
  `set_fit_bounds`; plan schematic-studio T27): a renderer made with several
  meshes draws only the chosen one and can frame a fixed box, so the studio
  draws a batch of block icons with one renderer, each framed alike.
  Guard: `lumilio-schematic-render` `a_batch_of_icons_draws_each_block_alone`.
- Focus mask (`src/rendering/gpu.rs` `set_focus`, `FocusMask`, `FocusMode`;
  `shader.wgsl` `in_focus`; plan schematic-studio T31, D30): a one-byte-per-
  cell 3D texture in the uniform group; the fragment shader finds the cell
  each surface closes (half a block back, as the mesher centres blocks) and
  keeps it, fades it toward the background or discards it. The mesh and the
  vertex format are unchanged. Guard: `lumilio-schematic-render`
  `a_focused_kind_keeps_its_colour_and_the_rest_fade_or_hide`.
- Cell outline (`src/rendering/gpu.rs` `set_mark`, `CellMark`; `shader.wgsl`
  `outline`; plan schematic-studio T32, D31): four uniforms give one cell,
  its block's box, the band's width in pixels and its colour. The fragment
  shader draws a band along the box's edges on each face of that cell's
  block, with a rim of the background inside it, from screen-space
  derivatives taken before any branch. Guards: `lumilio-schematic-render`
  `a_marked_block_is_outlined_and_nothing_else_changes`,
  `an_outline_follows_the_shape_of_its_block`.
- Slice and selection (`src/rendering/gpu.rs` `set_clip`, `Clip`,
  `set_selection`, `SelectionBox`; `shader.wgsl` `selected`; plan
  schematic-studio T25): a clip uniform discards the cells past one layer
  along an axis; a selection box washes its cells a little toward its
  colour and draws a band of it along the box's edges, the band as wide as
  the cell outline's (two pixels without one). Guards:
  `lumilio-schematic-render` `a_slice_shows_its_layer_from_above_and_clicks_land_on_it`,
  `a_selection_draws_a_band_around_its_box`.
- Entity state and placement by category (plan schematic-studio T35, D32;
  `src/meshing/entity_instances/mod.rs`). `placed`: a row with `aim` is drawn
  at the entity's exact position by `aimed_at`, a hung painting turned by its
  facing, a display entity's `transformation` (parts or sixteen numbers)
  inside the turn; a `billboard` row or a display's `billboard` makes the
  instance a `Billboard`, turned to the camera each frame in
  `rendering/instances.rs` `visible` from `Culling::back` (`instances::back`,
  set in `gpu.rs`). `add_variants` reads what the entity's save says by
  field: a carried block (`BlockState`, `block_state`, `DisplayState` with
  `CustomDisplayTile`, `DisplayOffset`), `RabbitType` and a rabbit named
  Toast, a tropical fish's packed `Variant`, a cushion's `color`,
  `dangerous`, `ShowBottom`, an old painting's `Motive`, an orb's `Value`, a
  display's text fields and item context, and the UUID that picks a
  mannequin's default skin. Guards: `lumilio-schematic-render`
  `tests/coverage.rs`.
- Entity instances (`src/meshing/entity_instances/`, `src/rendering/instances.rs`,
  `gpu.rs`, `shader.wgsl` `vs_instanced`; plan schematic-studio T8):
  `to_mesh_with_entity_instances` meshes the blocks without the schematic's
  entities, then meshes each distinct look of an entity (its id and every
  property, without place and yaw) once at the origin with the same mesher.
  Every entity becomes an instance: its exact position, turned by its yaw when
  its binding row turns with it (Schematic-Mesher's
  `Facing::turns_with_yaw`), else in its block as before. NBT variants
  (`variant`, a horse's `Variant`, an older cat's `CatType`, `VillagerData`)
  become the properties the binding table reads. `GpuRenderer::with_entities`
  keeps one instance buffer per model and draws all kept instances of a model
  in one call per layer; each frame drops instances outside the frustum or,
  with `set_entity_distance`, too far from the eye, and orders the rest
  farthest first. Entities no row or layer draws are listed in
  `EntityInstances::undrawable`. Guards: `entity_instances` tests
  (`unbound_and_layerless_entities_are_reported_not_drawn`,
  `place_and_yaw_do_not_split_a_look`, `bound_mobs_turn_by_yaw_and_others_keep_their_block`,
  `variant_textures_come_from_any_versions_nbt`), `rendering::instances`
  tests (frustum, distance and per-model ordering), and
  `lumilio-schematic-render`'s `entities_are_drawn_as_instances_and_unbound_ones_are_reported`.
- Baked packs (`src/meshing/mod.rs` `ResourcePackSource::from_baked`): the
  launcher's only way in, with the same version's entity layers beside the
  pack. The former JSON rewriting of 26.x jars
  (`resource_pack_compat.rs`) and the entity texture aliases
  (`with_mesher_texture_aliases`) are deleted; `from_file`/`from_bytes` load
  a pack as written. Wood-specific boat ids pass through to the mesher (its
  binding table reads the wood), and an older `boat`'s `Type` becomes its
  `wood`. Guards: `a_baked_pack_answers_for_renamed_entity_textures`,
  `block_entities_draw_from_the_entity_layers_handed_in`.
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
