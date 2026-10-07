# 0034 — The native schematic preview follows Minecraft's draw rules

- Status: accepted
- Date: 2026-10-07

## Context

ADR 0029 forked Nucleation and Schematic-Mesher so native rendering could be fixed here.
The maintainer then saw missing blocks, see-through ("hollow") blocks and Z-fighting. A
probe against the 26.3 client JAR, the maintainer's schematics and a test scene traced every
symptom to the forks, not to `lumilio-schematic-render`:

- Hollow: face culling judged opacity from geometry only, so copper grates hid what was behind
  them; all leaves culled each other; the GPU drew back faces.
- Z-fighting: coplanar faces of one block (redstone line and overlay, grass overlays) and
  back-to-back faces (carpet on glass) met at equal depth; depth used a standard Z range;
  translucent triangles were drawn in a fixed order without depth writes.
- Missing: 26.x Euler element rotations failed to parse; since 26.2 signs and beds are block
  models but the mesher still drew legacy entities with textures that no longer exist;
  schematics saved before a rename (`chain` → `iron_chain`) were never converted; failures only
  reached stderr.

The schemat.io browser viewer (its `SchematicViewer` bundle) culls back faces on every terrain
pass, sorts translucent quads back to front, derives occlusion from opaque sprites only and
keeps leaves out of same-type culling. Minecraft 26.3's own `CuboidRotation` (read from the
unobfuscated client) defines the Euler order and rescale.

## Decision

- Mesher: a full cube with see-through texture pixels hides no neighbour face but still darkens
  ambient occlusion; leaves do not cull each other; copper grates cull only the same block,
  like glass (Minecraft's `HalfTransparentBlock`).
- Mesher: element rotations accept both forms, with Minecraft's semantics (`Rz·Ry·Rx`, rescale
  by the largest rotated component); normals turn with the element.
- Mesher: signs and beds whose model has elements are drawn from the model alone.
- Mesher: within one block, a face lying in the plane of an earlier overlapping face with the
  same facing moves out by 1/1024 block per earlier face, fixing Minecraft's draw order.
- Renderer: back-face culling on all layers; reversed depth with an infinite far plane and
  `GreaterEqual` tests; opaque and cutout layers without blending; translucent triangles
  re-sorted back to front every frame.
- Adapter: a schematic is converted to the data version in the game JAR's `version.json`
  before meshing. Blocks the pack still cannot draw are listed by `Scene::undrawable_blocks`,
  and the preview names how many are missing, with their IDs under 技术详情.

## Consequences

Native previews of 26.x schematics match the game far more closely, and a block the game's
assets cannot draw is no longer lost silently. The forks now carry real behaviour changes that
upstream syncs must preserve (`forks/README.md`). A frame of the maintainer's schematics went
from about 2.1 ms to 2.7–3.5 ms, mostly translucent sorting. Inside-out elements (spawner, vault) now show only their inner
faces from inside, as in the game.

Not done: sign text for 26.2+ block-model signs (it was composited onto the removed entity
texture); stained glass of different colours still culls each other; Nucleation's item-model
export omits Euler rotations; poses do not move translucent sort keys.

## What shipped

Guards: `forks/schematic-mesher/tests/{face_culling,element_rotation,block_model_entities,
coplanar_faces,undrawable_blocks}.rs`, Nucleation's `reversed_depth_keeps_framing_and_flips_depth_order`
and `translucent_triangles_draw_farthest_first`, and the adapter and preview tests; each guard
was shown to fail with its fix disabled. A winding probe over all 10 057 block states of the
26.3 JAR found front faces consistent with normals except the intentionally inside-out models.
Nucleation's 976 unit tests pass. The maintainer's schematics render at 2.7–3.5 ms per
1800×1200 frame. Renders of the probe scene were compared by eye; the maintainer still has to
look at the real preview.
