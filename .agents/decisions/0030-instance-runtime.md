# 0030 — Change instance runtime and repair game files

- Status: accepted
- Date: 2026-10-05

## Context

A game needs a way to change its Minecraft version and loader, and to repair
corrupted game files, without leaving a half-applied state or rewriting release
artifacts other instances share. Installed mods may no longer fit after a
version or loader change.

## Decision

- `change_runtime` prepares the new combo's shared files first, then commits the
  instance record under the instance lease. Failure, cancel, or a failed commit
  rolls back to the previous combo, which stays launchable offline. Releases
  shared with other instances are not rewritten. `InstanceStore::set_runtime` is
  a single database transaction.
- `repair_instance` reuses the launch-time `InstallationVerifier` /
  `RepairExecutor`: damaged files are re-fetched, natives rebuilt; when already
  intact there is no network work.
- The settings runtime section offers「更换…」and「修复」. The change dialog
  starts from the current combo (the same combo cannot be submitted), warns that
  installed mods may be incompatible, and offers an optional「先建快照」. Activity
  and history record the change.

## Open follow-up

Snapshot before a change is suggested, not required.「先建快照」is a separate
action from submitting the change; a failed snapshot does not gate the submit.
Whether a snapshot failure should block continuing was listed as an open
question and has not been decided; the current behavior allows continuing.

## Consequences

- Positive: a failed or cancelled change does not leave a broken game; repair
  shares the same verification path as launch.
- Negative / trade-offs: compatibility of mods across loaders is not judged;
  modpack updates are out of scope.
- Follow-ups: decide whether a failed「先建快照」should block the change.

## What shipped

Core `change_runtime` and `repair_instance` with tests, the runtime settings
section and change dialog (warning and optional snapshot), app wiring, and
history. On 2026-10-05 the maintainer accepted the visual checks (runtime
section, change dialog, snapshot-first, repair). The implementation plan is
closed.
