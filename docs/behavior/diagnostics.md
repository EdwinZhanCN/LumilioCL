# Diagnostics behavior

## Problems

`diagnose` turns gathered facts into problems, most severe first.

- **Errors** (the instance cannot launch): loader not launchable yet; no
  account; no installed Java that satisfies the release's requirement (a newer
  Java counts; an older one does not).
- **Info**: not installed yet (the first launch installs).
- **Warnings**: damaged or missing installation files (count); enabled mods
  built for another loader (only checked for modded instances); the same mod id
  in more than one enabled file; a heap limit under 1024 MB; the last session
  crashed or failed to start. A session the user stopped is not a problem, and
  an unset memory limit is not a problem.
- Disabled mods and mods with unreadable metadata never produce mod problems.

## Logs

- The log view is the tail of `logs/latest.log`, cut on a line boundary; a
  missing log is "none". Crash reports are `crash-reports/*.txt`, newest first,
  read by file name only (never a path) and capped in size.
- `analyze` recognizes well-known failures in text: out of memory, Java too old
  for the classes, unrecognized JVM options, mod conflicts, graphics failures.
  Each hint appears at most once, in a fixed order.

## Files

Listing takes a `/`-separated relative path of plain names inside the game
directory; `..`, absolute paths, backslashes, colons and NUL are refused.
Folders come first, then names ignoring case. Links are not followed.

## Diagnostics bundle

`export_diagnostics` writes one zip through a temporary file (a failure leaves
nothing behind): version and system, a settings summary, the Java list, the
instance list, recent tasks and each instance's latest log. Player names,
profile ids (both spellings), the launcher folder and the user's home folder
are replaced by `<player>`, `<uuid>`, `<launcher>` and `~`. The pre-launch,
wrapper and post-exit commands, the arguments and the environment are never
included, only whether each is set.
