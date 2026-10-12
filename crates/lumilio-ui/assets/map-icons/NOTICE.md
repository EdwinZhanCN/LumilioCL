# Map icons: provenance and status

These are the structure-marker icons of
[MinecraftSearch](https://minecraftsearch.com), its seed map's
`/images/structures/` set. The files were fetched on 2026-10-09 and converted
from WebP to PNG without other changes (80×80, RGBA). This replaces the earlier
set, which was taken from the [Axolotl](https://github.com/Mystic-Stars/Axolotl)
launcher.

The set is the whole folder, including icons the map does not draw yet —
dungeons, caves, the sulfur spring and the mega ravines. Which ones the map
uses is the table in `crates/lumilio-ui/src/map_icons.rs`: the structures the
seed layer computes (village, the temples, igloo with and without basement,
ruins, shipwreck, monument, mansion, outpost, ruined portal and its Nether
form, ancient city, trail ruins, trial chambers, stronghold, nether fortress,
the four bastion remnants, nether fossil, End city, End gateway, abandoned
camp, mineshaft, buried treasure, desert well, amethyst geode), the fossils,
the lava lake (`underground_lava_lake.png`), the copper and iron veins, the
ravines, the enchanted golden apple, plus the world-spawn and slime-chunk
markers. An icon with no entry draws a plain dot; a file with no entry is
simply unused.

MinecraftSearch's About page says its block, mob and biome pictures come in
large part from the Minecraft Wiki (CC BY-NC-SA 3.0) and that it intends to
replace them with its own, and most of these images depict Mojang's Minecraft
content.

**Status: not licensed to LumilioCL.** Nobody has granted us a license for
these files. We use them for identification inside an unofficial launcher, on
the same footing as Axolotl, with the rights staying with MinecraftSearch, the
original creators and Mojang. Nothing here is released under the repository's
license (AGPL-3.0-only), and the files must not be described as CC BY-NC-SA or
as anything else.

Minecraft is a trademark of Mojang Synergies AB. LumilioCL is not affiliated
with or endorsed by Mojang or MinecraftSearch.

## Removal

Every use goes through `crates/lumilio-ui/src/map_icons.rs`, which falls back to
a plain dot for a missing icon. If a rights holder objects, or an original set
replaces these, delete this directory and the table in that file; nothing else
refers to the files. (Decision: maintainer, 2026-10-08; set replaced
2026-10-09.)
