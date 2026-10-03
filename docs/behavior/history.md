# History behavior

- Each instance has an append-only `history.jsonl` of events: play sessions and
  changes (content added/removed/enabled/disabled/updated, settings changed,
  game version changed, repaired).
- Reading skips lines it cannot understand (a torn final line, an event kind
  from a newer launcher) and reports how many were skipped. A missing log is
  empty.
- Views are newest first. `compact(n)` keeps the newest `n` events, atomically.
- A session's outcome: *stopped* if the user killed it; *failed to start* if it
  ended before the game finished starting; *clean* if it ran and exited 0;
  otherwise *crashed*. Launches without a process are *failed to prepare* or
  *cancelled* (no play time); a launcher that died mid-launch leaves
  *interrupted* (how the game ended is unknown).
- Finishing a session appends the event and adds the run time to the
  instance's play total and last-played time in one step.
