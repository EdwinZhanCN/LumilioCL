# Settings and accounts behavior

- One `settings.json` under the launcher root. It shares the persistence rules
  of every small launcher file (`persist`): atomic write, damaged file kept as
  `.broken` and the defaults used, newer-schema file refused untouched.
- Memory defaults are optional; each value must be 1..=1,048,576 MB and the
  minimum may not exceed the maximum. A rejected change leaves settings as they
  were. Instance settings override these defaults.
- Accounts are offline profiles (validated like `OfflineProfile`); names are
  unique ignoring case. The first account added becomes selected; removing the
  selected account selects the first remaining one, or none.
- An account may carry its own profile id (32 hex digits, dashes optional; the
  nil id is refused), set only when the account is added. It wins over the id
  derived from the name, must differ from every other account's id, and is
  stored as 32 lowercase hex digits. Files written before this field existed
  still load, with ids derived from names.
- `current_instance` remembers the instance the launcher plays and installs
  into. It is optional; the service reads it as none when that instance no
  longer exists, and the caller picks another and saves it.
- `preferences` hold launcher-only choices: appearance (system, light, dark),
  what the window does once the game runs (keep or hide — never close, because
  the launcher supervises the game to record its session), whether to come back
  to the front when it ends (default yes), and motion (system, reduce, full).
- `launch` holds the launch defaults (`LaunchTuning`): window width and height
  (both or neither, 1..=16384), fullscreen, JVM arguments, game arguments,
  environment variables, and the pre-launch, wrapper and post-exit commands.
  Blank commands and argument lines are dropped when saved. An instance's own
  JVM arguments replace the defaults'; everything else comes from the defaults.
- `download_concurrency` is 1..=32 or unset (the launcher decides).
- `disabled_java` lists installation homes the user turned off: they are shown
  but never chosen for a launch or a diagnosis.
- Mirrors rewrite official address prefixes. The download source order is the
  official address plus mirrors; `prefer_mirrors` puts mirrors first. Empty
  prefixes are rejected.

- Settings writes use a private candidate. The public projection (including
  selected account and schema) changes only after persistence succeeds; temporary
  write or publication failure preserves the last saved values and permits retry.
- Instance memory overrides use the same range/order checks. The service checks
  the resolved pair against current defaults when saving overrides. Clearing an
  override resumes inheritance. Changing defaults never silently edits instances;
  the service and launch pipeline reject an invalid resolved pair before downloads
  or process creation. Legacy values read from disk are checked again at launch.
- `LauncherService::settings` returns saved settings, `set_default_memory` updates
  defaults, and `update_instance_settings` commits validated instance overrides.
  These core APIs do not imply a global settings UI exists.

## Instance overrides

An instance keeps `launch` (`InstanceLaunch`) next to its memory, Java path and
JVM arguments. Each field is optional: absent follows the launcher default, so
restoring the default removes the override and never copies the current
default into the instance. An empty argument list, environment list or command
means "none on purpose" and is different from absent. The window, fullscreen,
game arguments, environment, the three commands, what the launcher window does
after launch, and a direct-start target are stored there; JVM arguments keep
their own column (empty follows the defaults). Every override is checked like
the defaults, and a refused change changes nothing. A chosen Java path must
exist when it is saved.

The library schema is 2. A schema-1 `launcher.db` is upgraded in place (a
`tuning` column is added) after a copy is kept as `launcher.db.v1`; a database
written by a newer launcher is still refused untouched.
