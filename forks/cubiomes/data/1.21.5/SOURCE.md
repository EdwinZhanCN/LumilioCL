# Minecraft Java 1.21.5 generation data

Server SHA1: `e6ec2f64e6080b9b5d9b471b291c33cc7f509733`.
Original server: https://piston-data.mojang.com/v1/objects/e6ec2f64e6080b9b5d9b471b291c33cc7f509733/server.jar
Release metadata: https://piston-meta.mojang.com/v1/packages/5617971be600cbdf93c60fc218b5073987f6507b/1.21.5.json

Rebuild using Java 25 (set WORLDGEN_JAVA and WORLDGEN_JAVAC when needed):

```sh
cargo xtask worldgen-prepare 1.21.5
cargo xtask worldgen-golden 1.21.5
python3 crates/lumilio-xtask/src/worldgen/save_golden.py 1.21.5
cargo xtask worldgen-save-read 1.21.5
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
