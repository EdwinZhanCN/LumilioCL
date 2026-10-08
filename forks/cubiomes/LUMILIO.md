# cubiomes snapshot

Upstream: https://github.com/Cubitect/cubiomes
Baseline: `e61f90580cbdd883214a8054670dacae655e59c0`.
License: MIT, Copyright (c) 2020 Cubitect; see [LICENSE](LICENSE).

Imported: generation C sources and headers, `rng.h`, `tables/`, README and LICENSE.
Omitted: `quadbase.*` (seed search and threads), `tests.c`, docs and build files.

## Local patches

- 2026-10-08: none. The host-owned bridge lives in `crates/lumilio-cubiomes/src/bridge.c`.
