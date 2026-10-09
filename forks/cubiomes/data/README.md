# Minecraft generation data

Each release directory records its original Mojang server hash and derived
numeric parameter tree. `diffs/` compares all bundled worldgen resources,
biome tags and original data-generator parameter reports. Decorative changes
in those reports do not necessarily change the biome map.

The maintained tables cover 1.21.5–26.1.2 (identical numeric trees), 26.2
(sulfur caves), and 26.3 (dappled forest and irregular tree branches).
1.21.4 is the unmodified upstream baseline used to validate the encoder.

Use Java 25, Python 3, curl and the normal Rust workspace environment:

```sh
export WORLDGEN_JAVA=/path/to/java25/bin/java
export WORLDGEN_JAVAC=/path/to/java25/bin/javac
cargo xtask worldgen-prepare 26.3
cargo xtask worldgen-golden 26.3
cargo xtask worldgen-diff 26.2 26.3
cargo xtask cubiomes-btree forks/cubiomes/data/26.3/trees.json.gz forks/cubiomes/tables/btree26_3.h btree26_3
python3 crates/lumilio-xtask/src/worldgen/save_golden.py 26.3
cargo xtask worldgen-save-read 26.3
```

`worldgen-golden` samples the original server's BiomeSource/RandomState at
3 seeds × 3 dimensions × 4 heights × 96 positions, exports the numeric search
tree, and samples structure region grids. `worldgen-import` reimports an
existing probe without starting Java. Goldens live in
`crates/lumilio-cubiomes/tests/golden/`; their biome coordinates are quart
coordinates, and their structure coordinates are blocks.

`save_golden.py` starts disposable, loopback-only vanilla servers with no
players. It generates real chunks at selected points, including new biomes,
and records vanilla `/locate` results. `worldgen-save-read` reads the saved
Anvil biome palettes and world spawn without using the runtime probe. Negative heights are
sampled only in the Overworld, where those sections exist.

Fixture rows use these fields:

- Biomes: `seed dimension quart_x quart_y quart_z biome_id`.
- Structure grids: `seed kind region_x region_z block_x block_z`.
- Biome rules: `kind biome_id allowed`; kind -1 is the stronghold bias tag.
- Saved locates: `seed dimension kind block_x block_z`; kind -2 is world
  spawn, -1 is stronghold, and other kinds are Rust `Structure` indices.

Dimensions are -1 (Nether), 0 (Overworld), and 1 (End). Camp locates validate
the exact region grid; their terrain-dependent viability remains estimated.

The oracle and saved worlds use the same original release but are independent
of cubiomes. They cover sampled behavior; they do not constitute the pending
maintainer F3 acceptance required by the world explorer plan.

Downloaded jars, mappings, renamed bytecode, logs and worlds remain in ignored
`target/worldgen`. No Mojang source code or game assets are committed. Numeric
ids reuse the MIT cubiomes baseline and append sulfur_caves=187 and
dappled_forest=188.
