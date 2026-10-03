# 0009 — Service owns the data-root writer lock

- Status: proposed
- Date: 2026-09-30

## Context

App State and L-OPS-01 require one writer per data root. SQLite transactions do not coordinate settings, profile files, or shared meta. The application creates one LauncherService for those writes.

## Decision

The implementation under plan 0015 acquires a nonblocking OS exclusive lock on locks/writer.lock before opening stores and retains the file handle for the service lifetime. Lock contention returns RootBusy. The lock file remains on disk; OS handle ownership determines exclusivity, including after process exit. Standard-library File locking avoids a new dependency.

## Consequences

- Independent data roots can run concurrently; aliases to the same lock file conflict.
- Restart tests must close the old service before opening the replacement.
- This protects cooperating LauncherService users, not arbitrary tools that write files directly. Low-level stores do not independently acquire another root lock.
- Shared-resource coordination inside one service, process-abort supervision, recovery UI and durable operation journals remain separate work.
- Status remains proposed for maintainer review; implementation follows the already documented single-writer target and does not imply acceptance of other proposed ADRs.
