# Reclaiming unused shared game files (ADR 0017)

- Settings › Storage › 检查没用的游戏文件 measures first (nothing is deleted), then asks, saying how much.
- A shared file is *unused* when no game's version needs it. A game needs its game-version folder and its loader's profile folder (Fabric/Quilt by exact name; Forge/NeoForge by name containing the loader and its version). From those versions' json: the libraries (by listed path or maven coordinates), the asset index and its objects, the logging config; natives are per version.
- Left alone on purpose, and said so: libraries when any game uses Forge/NeoForge (their installers make files no list names); asset objects when an index cannot be read. A needed version whose json cannot be read stops the whole scan.
- Removal refuses while any game is installing, repairing, launching or otherwise in use, scans again, and only removes real files or folders below `meta/` (no links, no `..`, parents resolved). Folders left empty are tidied away. Anything still wanted later is fetched again on the next launch.
