# 0016 — Importing games from other launchers

- Status: accepted
- Date: 2026-10-03

## Context

`docs/ia/library.md` lists "import instances from Prism, HMCL, CurseForge" as pending a format-scope decision. People moving launchers want their games, not just modpack files. A modpack file is a distribution format; an instance folder is a working game.

## Decision

Support two readable layouts, detected from a chosen folder (zip archives of instances are not read): a MultiMC/Prism instance (`instance.cfg` + `mmc-pack.json`, game files in `.minecraft` or `minecraft`) and a plain `.minecraft` folder (game version and loader read from its `versions/*/*.json`, preferring the one the person picks when several exist). The result is always a new game with a copy of the game directory; the source is never modified or moved. CurseForge's own formats are out of scope: they require the CurseForge API and a key.

## Consequences

- Positive: covers the two launchers whose data is local and documented.
- Negative / trade-offs: mods installed by other launchers are not re-identified until the content tab asks Modrinth; a loader version the launcher cannot install is kept in the record so the person is told.
- Follow-ups: exporting to the MultiMC/Prism zip layout is covered by the same format module.
