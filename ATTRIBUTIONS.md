# Native schematic preview dependencies

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
