# 0015 — Full instance backup and restore

- Status: proposed
- Date: 2026-10-03

## Context

Snapshots cover worlds and a few settings and live inside the launcher's data root. People also want one file holding a whole game (mods, config, resource packs, worlds, options) that they can keep elsewhere and restore later, including on another machine. Exported modpacks are for sharing and deliberately leave out worlds and personal files.

## Decision

A full backup is a zip with a small `lumilio-backup.json` (format version, the game's name, game version, loader and loader version, favourite is not kept) and the game directory under `game/`. Creating one takes the instance lease; links are skipped; logs, crash reports and the launcher's own bookkeeping are left out. Restoring creates a new game from the record and extracts into a staging folder published with one rename; it never replaces an existing game. The file is private data: it may hold account-bound player data but never credentials (those live in the system credential store).

## Consequences

- Positive: a simple, inspectable format; restore cannot damage an existing game.
- Negative / trade-offs: large files; restoring does not re-download anything, so the game still needs its shared game files installed on first launch.
- Follow-ups: backing up launcher settings and accounts is out of scope.
