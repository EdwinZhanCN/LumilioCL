# Block colour tables

Derived from the original game's textures; regenerate with the command in each section. The jar itself is not kept in the repository.

## 26.3

- Client jar sha1: `e877b6a07acd633fb3bb475002175cec036e7b87` (from Mojang's version metadata)
- Command: `cargo xtask block-colors 26.3`
- Tool commit: `666e1e2788fd051021b6f3f8aaf625e264b21a51`
- Blocks: 1251
- Colour: the mean of the visible pixels of the texture a block shows from above (`top`, then `up`, `all`, `end`, `side`); spruce and birch leaves and lily pads have the game's fixed tint multiplied in; flags 1 = see-through, 2 = grass tint, 4 = foliage tint, 8 = water tint; biomes give grass, foliage and water colour from the jar's biome files and colormaps

## 1.21.4

- Client jar sha1: `a7e5a6024bfd3cd614625aa05629adf760020304` (from Mojang's version metadata)
- Command: `cargo xtask block-colors 1.21.4`
- Tool commit: `666e1e2788fd051021b6f3f8aaf625e264b21a51`
- Blocks: 1094
- Colour: the mean of the visible pixels of the texture a block shows from above (`top`, then `up`, `all`, `end`, `side`); spruce and birch leaves and lily pads have the game's fixed tint multiplied in; flags 1 = see-through, 2 = grass tint, 4 = foliage tint, 8 = water tint; biomes give grass, foliage and water colour from the jar's biome files and colormaps

