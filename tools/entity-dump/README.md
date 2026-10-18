# entity-dump

`EntityDump.java` writes the entity model geometry of one Minecraft client jar
as JSON (plan `schematic-studio`, T6 and D4). It calls the game's
`net.minecraft.client.model.geom.LayerDefinitions.createRoots()` and walks each
model layer's `MeshDefinition`. Before that it calls the game's
`SharedConstants.tryDetectVersion()` and `Bootstrap.bootStrap()`, because some
layers read registries. It uses only Mojang's classes and the JDK.

The compiled class, `lumilio/entitydump/EntityDump.class`, is committed next to
the source, so a bake does not need a JDK compiler. The class has its own
package because the obfuscated jars are signed and keep their classes in the
unnamed package. `lumilio-block-assets` embeds the class (`DUMPER_CLASS`) and
the arguments it runs with. `cargo xtask bake-blocks` runs it for each release.
The launcher runs the same class for an unobfuscated 26.x client that the baked
file does not include yet, and caches the result by the jar's SHA-1 and the
class's SHA-1. Both keep the same canonical JSON (`EntityDump::to_json`).

## Rebuild

Run this command after each change to `EntityDump.java`. Use a JDK 21 or newer:

```sh
javac --release 21 -d tools/entity-dump tools/entity-dump/EntityDump.java
```

Commit the source and the class together.

## Run

```sh
java -cp tools/entity-dump:<client.jar>:<libraries…> lumilio.entitydump.EntityDump OUTPUT.json [NAMES]
```

- The class path contains the client jar and the libraries that the version
  metadata lists. On Windows, use `;` as the separator.
- The Java runtime must be version 21 or newer, and at least the
  `javaVersion` of the game version.
- `NAMES` is necessary only for obfuscated jars (1.17 to 1.21.x).
  `cargo xtask bake-blocks` makes this file from Mojang's official
  `client_mappings`. The dumper does not read mapping files.
  Each line maps an official name to the name in the jar:

  ```text
  class net.minecraft.client.model.geom.PartPose eqt
  field net.minecraft.client.model.geom.PartPose xRot d
  method net.minecraft.client.model.geom.LayerDefinitions createRoots a
  ```

## Output

```json
{"format":2,"layers":{"minecraft:chest#main":{"texture_size":[64,64],"root":{
  "pivot":[0,0,0],"rotation":[0,0,0],
  "children":{"lid":{"pivot":[0,9,1],"rotation":[0,0,0],
    "cubes":[{"origin":[1,0,0],"size":[14,5,14],"uv":[0,0]}]}}}}}}
```

The units are the game's units: 1/16 block, Y down, and radians. The keys are
sorted, so the same jar always gives the same bytes. The optional fields are
`scale`, `cubes`, `children`, `uv_scale`, `inflate`, `mirror` and `faces`. The
dumper writes them only when they are different from the default value.
`lumilio_block_assets::EntityDump` reads this format, and format 1, which
has only the layers.

### Renderers

From format 2 the dump also has `entities` (plan `schematic-studio`, T34). For
each entity type in the game's registry, the dumper builds the game's own
renderer. The renderer context holds only the model layers, and the layer map
records each layer that the renderer asks for. The output for each entity is:

```json
"minecraft:bee":{"renderer":["BeeRenderer","AgeableMobRenderer","MobRenderer","LivingEntityRenderer"],
  "entity":["Bee","Animal","AgeableMob","PathfinderMob","Mob","LivingEntity","Entity"],
  "layers":["minecraft:bee#main","minecraft:bee_baby#main"],
  "texture":"minecraft:textures/entity/bee/bee.png",
  "baby_texture":"minecraft:textures/entity/bee/bee_baby.png"}
```

- `renderer` and `entity` are the class chains, as official simple names.
  The entity class comes from the renderer's generic superclass.
- `layers` are in the order that the renderer asked for them. The main model
  is first.
- `texture` and `baby_texture` come from a new render state (1.21.2 and later).
  Older renderers get their textures from an entity instance, so the bake
  borrows them from a newer release.
- `texture_fields` are textures that the renderer keeps in fields (boats and
  minecarts).
- `error` tells why a renderer did not build. For example, the painting
  renderer needs a texture atlas. If no renderer builds, the dump has
  `entities_error`, and the layers are still in the dump.

For obfuscated jars, `NAMES` also maps each class, so that the class chains
use the official names.
