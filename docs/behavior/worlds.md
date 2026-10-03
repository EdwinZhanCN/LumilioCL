# Worlds behavior

- A world is a folder under `saves/` containing `level.dat`. Other folders and
  files are ignored; a missing `saves/` is empty.
- `level.dat` is gzip-compressed NBT (an uncompressed file is also read). Name,
  last-played time, game version and hardcore flag come from its `Data`
  compound. Input is bounded: 8 MB file, 64 MB decompressed, nesting depth 128.
- A `level.dat` that cannot be read still lists the world, named by its folder
  and marked damaged, so the user can act on it.
- Worlds sort by last played (newest first), then by name ignoring case.
- Deleting or duplicating takes a folder name, never a path; unsafe names are
  refused. Duplicating never overwrites, skips symbolic links, and removes a
  partial copy if it fails. `copy_name` proposes `<name> copy`, `<name> copy 2`, …
- Size is the sum of file sizes, not following symbolic links.
- `saves/` or a world folder that is itself a link is refused (`Linked`) for
  listing, deleting and duplicating: it could lead outside the instance. Links
  inside a world are never followed.
- A copy is built in a hidden `.<name>.copying` folder and published with one
  rename, so a crash never leaves something that looks like a finished world
  (hidden folders are not listed). `sweep_temporary` removes such leftovers.

## Through the service

- `worlds(id)` needs no lease. `copy_world(id, folder, new_name?)` and
  `delete_world(id, folder)` take the instance lease (refused with
  `InstanceBusy` while it launches or runs), sweep stale temporaries first when
  copying, and write one history entry (`WorldCopied` / `WorldDeleted`) only
  after the files changed. Deleting is permanent.
