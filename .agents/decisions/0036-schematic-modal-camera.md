# 0036 — Modal schematic observation and scoped pointer capture

- Status: accepted
- Date: 2026-10-07

## Context

The inline orbit preview is too small for exploring a build. The maintainer wants
the screenshot viewer's modal surface, with orbit and game-style first-person
controls. The locked GPUI 0.3.7 exposes focus and native window handles but no
relative-pointer lock API. Offscreen rendering already lives on a worker.

## Decision

- The page retains an entry key; each modal session owns a fresh viewer entity,
  worker and asset request identity. A late result cannot enter a reopened viewer.
- The entry joins the detail actions on the title row. Material tables expand
  in the shared detail scroll; a floating return-to-top key remains anchored
  outside it. Rendering children in their original tree order before arranging
  the actions preserves image and model identity.
- Orbital and Explore retain separate cameras. Explore starts outside the build
  at standing eye height and uses a 70-degree perspective, yaw-relative horizontal
  WASD movement, Space up and Shift down. Diagonals normalize and travel uses
  elapsed time. This is observation flight, without gravity or collision.
- Cursor capture is scoped by `lumilio-pointer::Capture`. Native cursor travel
  uses a scoped AppKit local-event monitor with mouse/cursor disassociation on
  macOS. Windows confines and recenters it to the client area; X11 uses a pointer
  grab and recentering. Hide/show is balanced on drop. All X11
  connection traffic stays on its own worker, never on the UI thread.
- Esc releases capture before closing. Window/viewport focus loss, mode changes,
  render failures and every modal dismissal release capture and held keys.
- Only captured input schedules frames. Camera response is intentional user motion
  and remains active under reduced motion; the world has no idle animation clock.
- Wayland rejects capture explicitly. It needs relative-pointer and pointer-
  constraint protocol integration with GPUI's connection/surface; pretending to
  capture with ordinary bounded mouse positions would not implement the contract.
- The new native-only crate uses safe CoreGraphics and X11 bindings. Its Windows
  module contains documented Win32 FFI and its macOS module a scoped AppKit event
  monitor bridge, each under a module-local unsafe allowance;
  the rest of the crate denies unsafe, and existing launcher/core/UI crates keep
  the workspace's `unsafe_code = forbid`. No core dependency on UI is introduced.

## Consequences

The page does no speculative rendering or asset loading, and closing frees its
scene and input resources. Native input requires hardware review on each platform;
Windows/X11 recentering uses OS cursor movement rather than raw HID deltas. Wayland capture
remains a platform adaptation task. Observer flight can pass through blocks.

## Implementation and validation

Modal, camera, scoped capture, IA and regression tests are implemented. Tests cover
lazy loading, reopened request identities, dismissal, independent camera modes,
movement heading, diagonal speed, elapsed time, opposite keys and cleared input.
Per the maintainer's explicit instruction, no local build/check/tests were run.
On 2026-10-07 the maintainer accepted the real cursor, focus and layout review:
light/dark, 720×480 and large windows, capture, look, WASD/Space/Shift, two-stage
Esc, focus changes, and cursor release after close and reopen. CI has not run
on this unpushed change.
