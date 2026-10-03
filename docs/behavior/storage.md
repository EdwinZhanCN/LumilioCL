# Storage layout behavior

```text
<root>/
  launcher.db            instances, collections (SQLite)
  meta/                  shared; each version is installed once
    libraries/ versions/ assets/ natives/<release-id>/
  profiles/<id>/         one isolated folder per instance
    game/ history.jsonl snapshots/
  runtimes/              Java runtimes the launcher owns
```

- `Layout` (core) is the only place these paths are spelled.
- **Shared, once per version.** Client jars, loader profiles, libraries, assets
  and natives are keyed by release id (`1.21.1`, `fabric-loader-0.16.0-1.21.1`)
  under `meta/`. A second instance of the same game version and loader finds
  everything installed: its launch skips the install phase (tested).
- **Isolated per instance.** Mods, saves, options, resource packs, history and
  snapshots live under `profiles/<id>/`. Nothing an instance changes is shared.
- Natives are extracted per release, so two instances with the same release
  share them; a different loader version has its own release id and folder.
- `launcher.db` schema version is SQLite's `user_version`. Each mutation
  rewrites the small tables in one transaction (a crash leaves the old or the
  new library). Data is a few dozen rows, so a full rewrite is simpler than
  row-level diffs. Mutations prepare a private candidate; the in-memory library
  is replaced only after the transaction commits. Write or commit failure leaves
  the last saved projection unchanged, and a later retry can succeed.
- Damaged database → `launcher.db.broken` (numbered if one exists), start empty, report. Newer schema →
  refuse, leave untouched.
- A `library.json` from before the database is imported on the first open of a
  fresh database and renamed `library.json.imported`. Older `instances/<id>/`
  folders and root-level `libraries/versions/assets` are **not** moved (no
  released build ever wrote them); they can be deleted.
- Removing an instance is journaled and recoverable: the profile is moved aside,
  the record committed, then the files removed; see [deletion](deletion.md).
  `meta/` is never touched by removal (other instances may need it); pruning
  unused releases is a later feature. Damaged files are kept, never overwritten
  (`.broken`, `.broken.1`…) and reported at start; see [recovery](recovery.md).

## Disk use and the cache

`measure` adds up four folders without following links: every instance's
profile (games), `meta/` minus `meta/natives` (shared), `runtimes/` and the
cache. The cache is `cache/` plus `meta/natives`; both can be made again, so
`clear_cache` deletes only them and reports the bytes freed. Games, shared
game files and Java runtimes are never touched.
