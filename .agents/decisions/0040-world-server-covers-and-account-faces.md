# 0040 — World and server covers, account faces

- Status: accepted
- Date: 2026-10-09

## Context

World and server rows had no image when the game or server supplied none. Account lists used an initial and a hash color even when a skin was available. Server icons can arrive both from a live status response and from `servers.dat`.

## Decision

Show a world's `icon.png` and prefer a server's live favicon over its cached `servers.dat` icon. Use the deterministic procedural cover when either image is absent. Keep validated server PNGs as data URLs in core; the app image client decodes them for GPUI. Render the front face and optional hat layer from an account skin, cache the result by account, and retain the initial when a face is unavailable. Legacy server versions use the legacy ping protocol selected from the instance's game version.

## Consequences

- World and server rows have a stable visual fallback; account faces arrive asynchronously without blocking the UI.
- Core validates untrusted server icons and handles old protocol versions; the UI chooses which image to show.
- The image treatment and row size still need a maintainer's visual review, tracked in `backlog.md`.

Shipped: world and server row images, account faces in navigation and Accounts, validated server icon loading, and legacy server ping.
