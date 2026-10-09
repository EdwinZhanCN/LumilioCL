# Native dependencies

## cubiomes

The world generation adapter vendors [Cubitect/cubiomes](https://github.com/Cubitect/cubiomes)
at `e61f90580cbdd883214a8054670dacae655e59c0` in `forks/cubiomes`.
Its biome palette (`util.c`, `initBiomeColors`) is largely inspired by Amidst.
The snapshot retains its original [MIT license](forks/cubiomes/LICENSE):

MIT License

Copyright (c) 2020 Cubitect

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## Native schematic preview dependencies

The schematic modal's scoped pointer adapter uses the existing MIT/Apache-2.0
`core-graphics`, `objc2` and `block2` bindings on macOS and MIT/Apache-2.0 `x11rb` bindings on X11.
Its Windows module is original Win32 API glue. No upstream source was copied
into `crates/lumilio-pointer` (ADR 0036).

The UI also maintains gpui-component 0.7.0 from Longbridge's GPUI Kit under
`forks/gpui-component`, with a local multi-select extension in
`crates/component/src/select.rs`. It is licensed under Apache-2.0; the complete
notice is preserved in [LICENSE-APACHE](forks/gpui-component/LICENSE-APACHE).
The baseline and imported paths are recorded in [fork maintenance](forks/README.md).

The native preview links the following libraries (ADR 0028). It does not vendor
Minecraft textures: a pack is built in memory from the player's installed game.

| Library | Locked revision | License | Source used |
| --- | --- | --- | --- |
| [Nucleation](https://github.com/Schem-at/Nucleation) | `51de345` | MIT, Copyright (c) 2025 Schem-at | `src/formats/litematic.rs`, `src/meshing/`, `src/rendering/{gpu,camera}.rs` |
| [Schematic-Mesher](https://github.com/Schem-at/Schematic-Mesher) | `286323e` | AGPL-3.0-only | Block geometry, liquid and chest meshing; linked through Nucleation |

Editable source snapshots are maintained under `forks/` (ADR 0029).
[Fork maintenance](forks/README.md) records complete baseline revisions, imported
paths and local changes. The original licenses are preserved at
[Nucleation/LICENSE](forks/nucleation/LICENSE) and
[Schematic-Mesher/LICENSE](forks/schematic-mesher/LICENSE). Mesher's AGPL-3.0-only
license matches LumilioCL's license. Nucleation's required notice follows.

## Nucleation — MIT License

Copyright (c) 2025 Schem-at

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

# Map icons

The icons in `crates/lumilio-ui/assets/map-icons/` come from the Axolotl
launcher, which took some of them from MinecraftSearch; the rest depict Mojang's
Minecraft content. They are **not** licensed to LumilioCL and are not covered by
this repository's license: the rights stay with MinecraftSearch, the original
creators and Mojang, and we use them for identification only, on the same
footing as Axolotl. Provenance, status and how to remove them are in
[crates/lumilio-ui/assets/map-icons/NOTICE.md](crates/lumilio-ui/assets/map-icons/NOTICE.md).
Minecraft is a trademark of Mojang Synergies AB; LumilioCL is not affiliated
with or endorsed by Mojang or MinecraftSearch.

# Player preview

`crates/lumilio-skin-render` draws the account preview on the CPU with its own
rasterizer. The elytra's wing dimensions, cape texture offset and resting angles
follow [skinview3d](https://github.com/bs-community/skinview3d)'s
`src/model.ts::ElytraObject` (MIT); its license is kept at
[licenses/skinview3d-MIT.txt](crates/lumilio-skin-render/licenses/skinview3d-MIT.txt).
Default player artwork is read from the player's installed client; none is
distributed. Which default an id gets follows HMCL's `TexturesLoader.java`
(GPL-3.0-or-later, ADR 0011).
