# Biome goldens

Input: cubiomes `e61f90580cbdd883214a8054670dacae655e59c0`, seed 262,
versions 1.16.5 / 1.18.2 / 1.21.4, Overworld / Nether / End, scales
1 / 4 / 16 / 64 / 256, scaled X/Z -2/-2, 4×4, block Y 64.
The upstream C probe produces 45 rows, with version, dimension and scale
as nested loops in that order. No Rust bridge participates.

From the repository root:

```sh
gcc -O2 -fwrapv -Iforks/cubiomes crates/lumilio-cubiomes/tests/golden.c forks/cubiomes/biomenoise.c forks/cubiomes/biomes.c forks/cubiomes/finders.c forks/cubiomes/generator.c forks/cubiomes/layers.c forks/cubiomes/noise.c forks/cubiomes/util.c -lm -o /tmp/lumilio-cubiomes-golden
/tmp/lumilio-cubiomes-golden > crates/lumilio-cubiomes/tests/biomes.txt
```

These verify adapter equivalence to the pinned library, not equivalence to
Minecraft saves. The latter remains human acceptance.
