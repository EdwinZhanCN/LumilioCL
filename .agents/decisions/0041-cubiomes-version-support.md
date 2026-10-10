# 0041 — Verify new cubiomes versions before exposing them

- Status: accepted
- Date: 2026-10-09

## Context

The maintained cubiomes snapshot stopped at Minecraft 1.21.4. Mapping later releases to the nearest known version could draw plausible but incorrect biomes and structures.

## Decision

Support Minecraft 1.21.5 through 26.3 with version-specific generation inputs and tables in the maintained fork. A future release enters the version table only after its generation inputs are compared, the relevant C implementation and reproducible data are updated, independent saved-world biome and `/locate` samples pass, and the maintainer checks the result in game. Unknown releases remain unsupported.

## Consequences

- The launcher can show seed maps for the verified releases without silently substituting another release's generation.
- Each future game release needs fresh evidence and fork maintenance. Spawn and terrain-dependent structure viability remain estimates where the upstream algorithm is approximate.

Shipped: 1.21.5–26.3 generation support, reproducible worldgen tools and goldens, plugin version integration, and maintainer game verification confirmed on 2026-10-09.
