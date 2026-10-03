# Activity log behavior

- The **task board** holds running tasks (category: download, install, update,
  repair; label; optional instance; progress). Ids are distinct. Progress never
  goes backwards or above a known total; unknown ids are ignored. The count of
  running tasks is the navigation badge.
- Finishing a task removes it from the board and appends it to
  `activity.jsonl` with its outcome (succeeded, failed with a message,
  cancelled). Finishing twice, or an unknown id, records nothing. A finish time
  before the start time is clamped to the start.
- History survives restarts. Reading skips lines it cannot understand and
  reports how many; views are newest first and can be limited to one category;
  `compact(n)` keeps the newest `n` atomically.
- Content/modpack service futures dropped before reporting a result finish as
  failed/interrupted; this does not claim file changes were rolled back. Normal
  completion records once. A hard process crash still cannot run this cleanup.
- If appending a terminal result fails, the board keeps it in memory and the
  service includes it in Activity, retaining the actual outcome. It does not
  resurrect the active task or automatically retry an ambiguous partial append.
  This fallback does not survive restart; durable recovery remains unfinished.
- Tasks still running when the launcher process crashes are not recorded.
