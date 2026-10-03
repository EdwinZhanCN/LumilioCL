# Full backup behavior (ADR 0015)

- Created from the game page ⋯ › 完整备份…. Takes the instance lease; it is an Activity task (cancellable, retryable). A cancelled or failed backup leaves neither the file nor a `.part`.
- The zip holds `lumilio-backup.json` (format 1, name, game version, loader and loader version, created, the game's settings) and the game folder under `game/`. `logs/` and `crash-reports/` are left out; links are never followed.
- Restoring (Library ⋯ › 从备份恢复…, or importing/dropping the zip like a pack) reads the json first (a zip without it, or from a newer format, is "not a backup"), builds the game in a staging folder, and publishes it with one rename. It always makes a **new** game; a game with the same name gets "（恢复）" added. The Java path in the saved settings is dropped (it belongs to the old machine). The game is "not installed": its shared game files are fetched on first launch.
- Entries that would leave the game folder are skipped; the declared size is capped.
