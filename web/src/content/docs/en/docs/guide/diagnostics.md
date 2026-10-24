---
title: Diagnostics
description: Export a diagnostics bundle, and see what it contains and what it hides.
---

When you ask for help or report a problem, you can export a diagnostics bundle.
The bundle is a `.zip` file.
The launcher does not send it automatically. You decide who gets it.

## Export the bundle

1. Open **Settings › About**.
2. In the **Diagnostics** row, click **Export…**.
3. Select a location for the file.

When the launcher saves the file, a message shows its location.

## Contents of the bundle

| File | Contents |
| --- | --- |
| `about.txt` | The launcher version, the operating system and architecture, the number of games, and the number of accounts |
| `settings.txt` | The default memory, the interface preferences (language, appearance, theme, fonts, wallpaper, translation, and others), the download concurrency, the number of mirrors, the download source, and the default window size and full screen setting of games |
| `java.txt` | Each Java that the launcher found: its version, vendor, architecture, off state, and location |
| `instances.txt` | Each game: its ID, game version, loader and loader version, installation state, and play time |
| `activity.txt` | The last 50 tasks that finished, with their results |
| `logs/` | The `latest.log` file of each game, and the output of the game from its last launch |
| `launcher/` | The log of the launcher, and the previous log if the current log became full |

For Java arguments, game arguments, and environment variables, the bundle contains only the number of items.
For the **Before launch**, **Wrapper**, and **After exit** commands, the bundle tells only if each command is set. It does not contain the commands.

## Hidden contents

When the launcher exports the bundle, it replaces these items with placeholder text:

| Item | Replacement |
| --- | --- |
| The player name of each account | `<player>` |
| The UUID of each account | `<uuid>` |
| The data folder of the launcher | `<launcher>` |
| Your home folder | `~` |

## Limits

- Paths that are not in your home folder stay as they are. An example is the location of a Java in a system folder.
- Game logs can contain chat text and server addresses. The launcher does not hide them.
- Before you share the bundle, you can open the `.zip` file and examine its contents.
