# Start-up recovery behavior

`LauncherService::open` settles earlier damage and exposes the result as
`startup_notes()` (plain `RecoveryNote` facts; the interface writes the words).
The app prints them to stderr until Diagnostics shows them.

| Situation | Behavior | Note |
|---|---|---|
| Interrupted deletion, library still has the instance | profile moved back, journal removed | `DeleteRolledBack` |
| Interrupted deletion, library no longer has it | leftover files and journal removed | `DeleteCompleted` |
| Both profile and quarantined copy exist | nothing touched | `DeleteConflict` |
| Library has it but no profile anywhere | record kept | `ProfileMissing` |
| Cleanup fails again / journal unreadable or newer | kept as is | `DeleteStuck` / `JournalUnusable` |
| `launcher.db` unreadable | original kept as `launcher.db.broken` (`.broken.1`… if one exists; never overwritten), library starts empty, profile folders the library does not know are listed | `LibraryRecovered` |
| `launcher.db` from a newer schema | open is refused, file untouched | error `NewerSchema` |
| `settings.json` unreadable | original kept the same way, defaults used | `SettingsRecovered` |
| Pack import or copy interrupted | staging folder removed, finished, or discarded per the table in [staged instances](staged-instances.md) | `PublishCompleted` / `PublishDiscarded` / `PublishStuck` |
| Snapshot restore interrupted | units put back from disk state, folder removed; a recreated target is never overwritten | `RestoreRolledBack` / `RestoreStuck` |
| Launch marker `state/sessions/<id>.json` left behind (the launcher died mid-launch) | history gets an *interrupted* session, marker removed, nothing re-attached or killed | `SessionInterrupted` |
| Torn lines in `activity.jsonl` | readable lines kept, count reported | `ActivityLogSkipped` |

Profile folders found on disk are leads, not a restored library: names, loaders
and settings are not guessed. A missing profile never deletes its record.
Per-instance `history.jsonl` already reports its skipped-line count when read.
Recovery runs again on every start, so stuck items are reported until fixed.

Not built: a recovery-mode screen, restoring records from candidates, migration.
