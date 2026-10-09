# Map icons: provenance and status

These 20 icons (80×80 PNG) are the structure and marker icons of the
[Axolotl](https://github.com/Mystic-Stars/Axolotl) launcher, commit
`b957550cff0541e972435e81dd6a8693763d69b3`, `apps/app-frontend/public/seed-map-assets/structures/`.
They were converted from WebP to PNG without other changes. Only the ones the
map draws are kept; Axolotl has seven more (geode, buried treasure, desert
well, mineshaft, end gateway and two end-city variants) that we left out.

Axolotl states in its `apps/app/COPYING.md` that some structure icons were
sourced from [MinecraftSearch](https://minecraftsearch.com), that MinecraftSearch
and the respective creators retain their rights in that artwork, and that the
remaining images depict Mojang's Minecraft content, used for identification in an
unofficial tool. MinecraftSearch's About page says its block, mob and biome
pictures come in large part from the Minecraft Wiki (CC BY-NC-SA 3.0) and that
it intends to replace them with its own.

**Status: not licensed to LumilioCL.** Nobody has granted us a license for
these files. We use them the way Axolotl does, on the same footing and with the
same risk: for identification inside an unofficial launcher, with the rights
staying with their owners (MinecraftSearch, the original creators, Mojang).
Nothing here is released under the repository's license (AGPL-3.0-only), and
the files must not be described as CC BY-NC-SA or as anything else.

Minecraft is a trademark of Mojang Synergies AB. LumilioCL is not affiliated
with or endorsed by Mojang or MinecraftSearch.

## Removal

Every use goes through `crates/lumilio-ui/src/map_icons.rs`, which falls back to
a plain dot for a missing icon. If a rights holder objects, or an original set
replaces these, delete this directory and the table in that file; nothing else
refers to the files. (Decision: maintainer, 2026-10-08.)
