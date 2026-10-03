# Content behavior

Mods, resource packs and shaders live in `mods/`, `resourcepacks/` and
`shaderpacks/` inside an instance's game directory.

- Mods are `.jar` files. Resource packs and shaders are `.zip` files or folders.
  Everything else and hidden files are ignored; a missing folder is empty.
- A file with the `.disabled` suffix is switched off; the display name drops
  the suffix. Items sort by display name ignoring case.
- Enabling/disabling renames the file; asking for the state it is already in
  changes nothing; a name collision with the other state is a conflict and
  overwrites nothing.
- Operations take a file name, never a path: empty, `.`/`..`, separators, colons
  and control characters are refused, so nothing outside the folder is touched. Removing a
  folder pack removes it recursively.
- A mod's identity comes from its archive: `fabric.mod.json`, `quilt.mod.json`,
  `META-INF/neoforge.mods.toml`, then `META-INF/mods.toml`. A version that is
  still a build placeholder (`${…}`) is treated as unknown. Unreadable or
  anonymous archives have no metadata (never an error).
- The SHA-1 of a file is what Modrinth identifies files by.
- A kind folder (`mods/`, `resourcepacks/`, `shaderpacks/`) that is itself a link
  is refused for every operation (`EscapesInstance`): it could lead outside the
  instance. A link *inside* the folder is only a file name; removing or renaming
  it acts on the link, never its target.

## Through the service

- `content(id, kind)` lists items and needs no instance lease.
- `set_content_state(id, kind, names, enabled)` and `delete_content(id, kind,
  names)` take the instance lease, so they are refused (`InstanceBusy`) while the
  instance launches, runs or installs. Each name gets its own result; one failure
  does not hide the others. Asking for the current state is `Unchanged`: nothing
  is touched and no history is written. A change writes one history entry
  (`ContentEnabled`/`Disabled`/`Removed`); if that write fails the file result
  still stands and `recorded` is false.
- `install_content` never replaces a same-named file whose SHA-1 differs from the
  published one (`Conflict`); identical content is reused. Replacing is a
  deliberate delete first. A cancelled or failed download leaves no destination
  and no temporary file behind.
