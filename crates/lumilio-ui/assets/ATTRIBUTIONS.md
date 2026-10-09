# Embedded icon attributions

LumilioCL embeds the following small set of SVG assets. The files are kept in
the repository so the launcher can start without a network connection.

## Phosphor Icons

Source: [phosphor-icons/core](https://github.com/phosphor-icons/core),
`assets/regular/`:

- `house.svg`
- `books.svg`
- `compass.svg`
- `pulse.svg`

License: [MIT](https://github.com/phosphor-icons/core/blob/main/LICENSE).

Copyright (c) 2023 Phosphor Icons.

The MIT License (MIT)

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

## Lucide Icons

Source: [lucide-icons/lucide](https://github.com/lucide-icons/lucide),
`icons/`:

- `download.svg`
- `plus.svg`
- `play.svg`
- `refresh-cw.svg`
- `info.svg`
- `arrow-right.svg`
- `x.svg`
- `square.svg`
- `star.svg` (`star-filled.svg` is the same path, filled)
- `chevron-left.svg`
- `search.svg`

License: [ISC](https://github.com/lucide-icons/lucide/blob/main/LICENSE).

Copyright (c) 2026 Lucide Icons and Contributors.

ISC License

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

Some Lucide icons are derived from the Feather project. The Lucide license
file lists the applicable Feather-derived icon names and the MIT notice; the
selected files above remain governed by the notices in that upstream file.

## Lucide (additions)

`heart.svg`, `clock.svg`, `chevron-right.svg`, `external-link.svg`, `ellipsis.svg`,
`chevrons-up-down.svg`, `chevron-down.svg`, `trash-2.svg`, `ban.svg`, `lock.svg`,
`chevron-up.svg`, `maximize.svg` under `icons/lucide/` are from [Lucide](https://lucide.dev), ISC license, like
the other Lucide icons listed above.

## Fonts

The interface's brand typefaces (design language §13) are embedded from
`assets/fonts/` and registered from memory at start-up. All three are licensed
under the [SIL Open Font License 1.1](https://openfontlicense.org); each licence
file sits beside the fonts.

- **Space Grotesk** (Light, Regular, Medium, Bold),
  [floriankarsten/space-grotesk](https://github.com/floriankarsten/space-grotesk).
  Copyright 2020 The Space Grotesk Project Authors. `OFL-SpaceGrotesk.txt`.
- **JetBrains Mono** (Regular, Medium),
  [JetBrains/JetBrainsMono](https://github.com/JetBrains/JetBrainsMono).
  Copyright 2020 The JetBrains Mono Project Authors. `OFL-JetBrainsMono.txt`.
- **DSEG7 Classic** (Regular), [keshikan/DSEG](https://github.com/keshikan/DSEG).
  Copyright (c) 2017 keshikan, Reserved Font Name "DSEG". `OFL-DSEG.txt`.

Chinese text is not bundled; it falls back to the platform's system font.

## Litematica 3D preview viewer

`litematic-viewer/` vendors [schematic-renderer](https://github.com/Schem-at/schematic-renderer)
1.6.1 (AGPL-3.0-only), [three.js](https://github.com/mrdoob/three.js) 0.184.0 (MIT) and
[nucleation](https://github.com/Schem-at/Nucleation) 0.2.18 (MIT), unmodified, so the preview opens
without a network connection (ADR 0027). The license texts are next to the files, and
`litematic-viewer/README.md` says what is included, what was left out on purpose (Mojang's default
textures) and how to update.
