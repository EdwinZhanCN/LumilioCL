# 0017 — Reclaiming unused shared game files

- Status: accepted
- Date: 2026-10-03

## Context

Game files shared between games (libraries, assets, versions) live under `meta/` and are never removed, so changing versions or deleting games leaves files nobody uses. Plan 0015 made deletion of shared files conditional on a read-reference guard.

## Decision

Measure first: a scan works out which shared files the installed games' releases still reference, and reports how much is unused. Removing them is a separate, confirmed step that takes the data-root writer lock, re-scans, and refuses if any game is installing, repairing or launching. Files that cannot be classified are kept.

## Consequences

- Positive: space can be recovered safely; the number is shown before anything is deleted.
- Negative / trade-offs: a game whose release metadata is missing makes its files look unreferenced, so such a game blocks removal; the next launch re-downloads anything removed that is still wanted.
- Follow-ups: automatic scheduling is out of scope.
