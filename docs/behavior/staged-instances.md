# Staged instances (pack import and copy)

A new instance's game folder is built completely before the instance exists.

1. Choose the id (`suggest_id`: unused in the library, no profile folder on disk).
2. Build `state/operations/<import|copy>-<id>/game/` (overrides and downloads, or
   a file copy). The library is **not** locked meanwhile.
3. Write `staged.ok`, then create the record (`create_as`, which refuses an id
   taken since step 1), then rename `game/` to `profiles/<id>/game` and remove the
   operation folder.

Any failure or cancel before step 3 removes the staging folder and changes
nothing else; a failed publish removes the record again. Existing instances are
never touched. If the id was taken meanwhile (same name created at once) the
operation fails with `IdTaken` and can simply be repeated.

Start-up recovery (`PublishCompleted` / `PublishDiscarded` / `PublishStuck`):
no `staged.ok` → remove; ready, record exists, game folder missing → finish the
rename; ready, both exist → remove garbage; ready, no record → discard and
report. Profile folders that start with `.` are never offered as recovery
candidates.

## Pack import

`import_modpack_file(path)` takes a local `.mrpack` (must be a regular file, and
is never modified or deleted); `install_modpack` downloads one and removes only
its own download. Both use the same rules as [modpack](modpack.md) and show as an
Install task in Activity that `cancel_task` can stop. Dangerous paths, a corrupt
archive, no trusted download source, a size/hash mismatch and a cancelled
download all leave nothing behind.

## Instance copy

`copy_instance(source, new_name, include_worlds)`:

- Copies the game folder as independent files (no hard links, symbolic links
  skipped). `logs/` and `crash-reports/` never come along; `saves/` only when
  `include_worlds`.
- The copy shares the source's release (`meta/` is not copied), inherits loader,
  versions, instance settings and the installed flag, and starts with no
  favorite, last-played, play time, history, snapshots or collections.
- Refused (`InstanceBusy`) while the source launches, runs or is being changed;
  the source is only read. Not part of the copy: a source whose game folder is a
  link is refused.
