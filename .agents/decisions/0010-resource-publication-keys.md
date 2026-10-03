# 0010 — Resource publication keys

- Status: proposed
- Date: 2026-09-30

## Context

A single root writer does not stop separate operations in that writer from replacing the same library, asset, native directory or release manifest. Different version installs also share some artifacts.

## Decision

Plan 0015 implements process-wide weakly held async mutexes keyed by canonical target paths. Downloads reserve their file before an engine concurrency slot and verify existing content after acquiring the key. Installer reserves sorted native-directory and release-manifest keys for execute; nested transfers own distinct file keys. Cancellation/drop release waiting or owned guards.

## Consequences

- Different file targets remain concurrent; separate engines share coordination.
- No new dependency or cross-UI state is introduced. Cooperative root ownership remains the cross-process boundary.
- This is publication writer coordination, not a claim of game resource read leases or crash-recoverable commits. Those remain required follow-ups.
- Filesystem aliases are resolved through an existing ancestor; caller paths must remain stable while the operation runs. External file writers cannot be made safe by an in-process mutex.
- Status remains proposed for maintainer review, documenting the already specified keyed-resource target implemented in 0015.
