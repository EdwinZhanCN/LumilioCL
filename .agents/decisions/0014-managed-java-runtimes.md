# 0014 — Launcher-managed Java runtimes

- Status: accepted
- Date: 2026-10-03

## Context

A game cannot start without a suitable Java. Today the launcher only finds installations the person already has (including its own `runtimes/` folder, which is searched). People without Java are told to install one themselves. Mojang publishes the runtimes its own launcher uses as a public index (`java-runtime/all.json`) listing, per platform, each component's file manifest with sizes and SHA-1s.

## Decision

Add an explicit "install Java" action (the problem button on a game, and Settings › Java). It reads Mojang's runtime index, picks the component for the needed major version on this platform, downloads every file through the normal transfer engine (size and SHA-1 verified), builds it under `runtimes/.<component>.installing` and publishes it with one rename to `runtimes/<component>`. It runs as an Activity task. Nothing is downloaded unless the person asks.

## Consequences

- Positive: a game can be made playable without leaving the launcher; the runtime is found by the existing Java discovery.
- Negative / trade-offs: depends on an external index format Mojang can change; macOS bundles are flattened to `Contents/Home` so the existing locator finds them; no automatic update of an installed runtime.
- Follow-ups: offering it automatically at launch is a separate UX decision.
