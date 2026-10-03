# 0006 — SQLite library, isolated profiles, shared `meta/`

- Status: accepted
- Date: 2026-09-30

## Context

The instance library was a single JSON file and every path was spelled where it
was used. The maintainer wants storage similar to Modrinth's launcher: SQLite for
launcher data, each instance isolated in `profiles/`, and game versions (with
their mod loader versions) stored once in `meta/` so two instances of one version
never install it twice.

## Decision

- `launcher.db` (SQLite via `rusqlite`, bundled) holds instances and
  collections. Small JSON files (settings) and JSONL logs (history, activity)
  keep their formats: they are not relational and already share `persist` rules.
- `Layout` in core is the single definition of paths: `meta/{libraries,versions,
  assets,natives/<release>}` shared; `profiles/<id>/{game,history.jsonl,
  snapshots}` per instance.
- Loader releases are ordinary releases in `meta/versions` keyed by
  `<loader>-loader-<loader version>-<game version>`, so they are shared like
  vanilla ones.

## Consequences

- Positive: no duplicate installs; instance deletion is a single folder; one
  place to change paths; schema versioned by `user_version`.
- Negative / trade-offs: a bundled C SQLite adds build time; `meta/` grows until
  a prune feature exists; `library.json` needs a one-time import.
- Follow-ups: prune unused releases; move history into the database only if
  cross-instance queries need it.
