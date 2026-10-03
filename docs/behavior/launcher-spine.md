# Launcher spine behavior

Contract for the modules between "pick a version" and "the game is running".
Mapping IDs `CORE-CATALOG-*`, `CORE-ACCT-*`, `CORE-JAVA-*`, and `CORE-PROC-*`
identify the read-only references; the design is LumilioCL's own.

## Version catalog (`catalog`)

The catalog is one JSON document listing versions with id, kind, release time,
manifest address, and optional checksum, plus `latest` pointers.

- A document without a `versions` list is invalid. Entries with a blank id or
  no manifest address are dropped, not fatal.
- Duplicate ids keep the first occurrence.
- Kinds: `release`, `snapshot`, `old_beta`/`old_alpha` (both `Old`); anything
  else is `Other` so a new upstream kind never breaks decoding.
- Entries are ordered newest first by release time (falls back to the `time`
  field, then empty). Equal times keep catalog order.
- Fetching goes through the configured `SourceChain`: the first source that
  answers 2xx wins; all failures are reported together.

## Offline account (`account`)

- A profile name is 1–16 ASCII letters, digits, or underscores after trimming.
- Unless the account carries a chosen id, the profile id is the MD5 name-based
  (version 3) UUID of
  `OfflinePlayer:<name>`, the same id an offline-mode server derives, so a
  player keeps their inventory across launchers.
- The launch session presents `msa` as the user type, the id as the token, and
  empty client/xuid values.
- Microsoft sign-in is a separate kind of account: see `accounts.md`.

## Instance store (`instance`)

- `launcher.db` (SQLite) under the launcher root holds instances and
  collections; each save is one transaction. Layout: `docs/behavior/storage.md`.
- Instance ids are ASCII slugs of the name (`instance` when nothing survives),
  made unique with `-2`, `-3`, … suffixes; an id is never reused for a live
  instance and never changes on rename.
- Unreadable database: kept as `launcher.db.broken`, the library starts empty
  and reports the recovery. Database from a newer schema: refused, untouched.
- Removing an instance removes its collection memberships but not its files.
- Shared files (libraries, versions, assets, natives per release) live in
  `meta/`; the game directory is the instance's own `profiles/<id>/game`.

## Java (`java`)

- A runtime is a directory with a `release` file (version, vendor,
  architecture) and a `bin/java` launcher. macOS bundles
  (`<name>/Contents/Home`) count.
- `1.8.0_412` is major 8; `17`, `21.0.3`, `25-ea` are their own majors.
- Discovery is deduplicated by real path and ordered newest major first.
- Selection: an explicit user-chosen runtime wins; otherwise the exact major
  the release requires, else the lowest newer major, else nothing. Without a
  requirement, Java 8 if present, else the lowest.

## Game process (`process`)

- Command order: Java, `-Xmx`/`-Xms`, default safety properties, the user's JVM
  arguments, the plan's JVM arguments, main class, game arguments.
- The high-level launch pipeline rejects out-of-range or contradictory resolved
  memory settings before installation or process creation. Low-level command
  formatting still normalizes its inputs as described below.
- The launcher's memory flags and default properties are omitted when the user
  already sets them. A minimum above the maximum is dropped.
- The game is *running* when it prints its start marker or has stayed alive for
  the settle time, whichever comes first. Ending before that is a failure
  (`ExitedEarly`); ending after is a normal exit.
- Cancelling kills the process. After it ends, output is collected for a short
  bounded time so a leftover grandchild holding a pipe cannot stall the launcher.
