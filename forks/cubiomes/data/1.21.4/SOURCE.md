# Minecraft Java 1.21.4 generation data

Server SHA1: `4707d00eb834b446575d89a61a11b5d548d8c001`.
Original server: https://piston-data.mojang.com/v1/objects/4707d00eb834b446575d89a61a11b5d548d8c001/server.jar
Release metadata: https://piston-meta.mojang.com/v1/packages/010c66f4559d4647c282d581a451cda6d45777db/1.21.4.json

Rebuild using Java 25 (set WORLDGEN_JAVA and WORLDGEN_JAVAC when needed):

```sh
cargo xtask worldgen-prepare 1.21.4
cargo xtask worldgen-golden 1.21.4
python3 crates/lumilio-xtask/src/worldgen/save_golden.py 1.21.4
cargo xtask worldgen-save-read 1.21.4
```

`trees.json.gz` is numeric parameter-tree data exported from the original
server runtime by `Probe.java`, including original child order and bounds.
The matching golden file contains 3 seeds × 3 dimensions × 4 heights × 96
positions, in quart coordinates (scale 4), sampled by that same original
server runtime's BiomeSource/RandomState. Regional structure candidates and
changed biome tags are recorded separately. Saved-chunk biome samples and
vanilla `/locate` results are produced by the last two commands. These
checks are independent of cubiomes and do not replace human F3 acceptance.

Game jars, mappings, and renamed bytecode remain in ignored target/worldgen.
No Mojang source or game assets are committed. Biome ids are LumilioCL's
ids from the MIT cubiomes baseline plus sulfur_caves=187 and dappled_forest=188.
