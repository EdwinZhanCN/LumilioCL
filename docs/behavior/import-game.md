# Importing games from other launchers (ADR 0016)

- Library ⋯ › 导入其他启动器的游戏…: choose a folder. Understood: a MultiMC/Prism instance (`instance.cfg` + `mmc-pack.json`, files in `.minecraft` or `minecraft`), and a Minecraft folder (`versions/*/*.json`; the folder holding `.minecraft` works too). A MultiMC/Prism instance **zip** can also be imported like a modpack (it is unpacked to a scratch folder under `cache/`, read, imported, and the scratch folder removed).
- A Minecraft folder may hold several versions; each readable one is offered (the first starts ticked). Version ids of Fabric, Quilt, Forge and NeoForge are taken apart; other modded ids are not guessed. A version folder that holds its own `mods`/`saves`/`config`/`options.txt` is that game's folder.
- Importing copies the player's files (mods, config, saves, options…) and leaves behind the other launcher's own folders (`versions`, `libraries`, `assets`, `runtime`, `logs`…), hidden entries and links. The source is never changed. A modded game needs a known loader version or it is refused.
- The result is a new game, "not installed"; it is built whole in a staging folder and published with one rename, so a failure or cancel leaves nothing.
- CurseForge's own formats are not supported.
