# Snapshots behavior

- A snapshot is a zip under `profiles/<id>/snapshots/` with a `snapshot.json`
  manifest (label, creation time, scope). Scope is *full* (all worlds plus
  `config/`, `options.txt`, `servers.dat`) or *one world* by folder name.
- Ids are `snapshot-<time>` (with `-2`, `-3` for the same second). Creation
  writes a temp file and renames, so a failure leaves nothing behind. Symbolic
  links are not archived. Nothing to back up is an error.
- Listing is newest first and skips files that are not readable snapshots.
- Restore works in `state/operations/restore-<instance>-<snapshot>/`: the whole
  archive is staged into `new/` first (a failure there leaves the game
  untouched), a `restore.json` journal lists the units, then each restorable
  unit (`saves/<world>`, `config`, `options.txt`, `servers.dat`) has its original
  moved into `old/` and the staged copy renamed into place. Worlds and settings
  not in the snapshot are kept; a restored world is replaced, not merged.
  Entries outside those units, or that would escape the game directory, are
  ignored. A total uncompressed size above 64 GiB is refused.
- If any step fails, every unit is put back as it was and the operation folder is
  removed; the snapshot remains for a retry. If even the rollback fails,
  `RestoreIncomplete` names the kept folder. The journal is removed first on
  success, so a leftover folder after a finished restore is only garbage.
- If the launcher dies mid-restore, the next start rolls the units back from the
  disk state alone (staged copy still in `new/` = not placed; gone = placed) and
  reports `RestoreRolledBack`. A target recreated in the meantime is never
  overwritten: the folder is kept and `RestoreStuck` is reported. A folder with no
  journal means the game was never touched and is removed. A pending operation
  blocks a new restore of the same snapshot (`RecoveryPending`).
- This is rollback-on-failure, not an atomic restore: while it runs the game
  folder is in an intermediate state, protected only by the instance lease.

## Through the service

`snapshots(id)` needs no lease; `create_snapshot`, `restore_snapshot` and
`delete_snapshot` take the instance lease (refused with `InstanceBusy` while it
launches or runs) and write one history entry each (`SnapshotCreated` /
`SnapshotRestored` / `SnapshotDeleted`) only after the files changed. Deleting is
always an explicit user action.
