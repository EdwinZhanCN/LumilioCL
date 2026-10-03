# 0012 — One page anatomy, a three-zone navigation bar, Accounts and Settings landmarks

- Status: accepted
- Date: 2026-10-01

## Context

Library, Discover, Activity, the Instance page and the project detail were
built in different plans and each arranged its header, actions, tabs, search
and context differently: Discover put search on its own row and the install
target as a line of chips; the Instance and project pages drew their own back
buttons; the Instance settings edited memory inline with two side-by-side
fields and a paragraph of help. The maintainer reviewed the pages and set one
information architecture for every page except Home.

Two further facts force scope changes:

- People need to manage offline and Microsoft accounts and launcher-wide
  settings. Neither is a node in `ARCH.md`; core already has
  `LauncherSettings` with accounts and default memory.
- Which instance "play" and Discover installs target is page-independent
  state (HMCL: H-NAV-03 "切换当前实例"), but today it is shown only on
  Discover.

## Decision

1. Every page except Home follows the page anatomy in
   `docs/design-language.md` §7 (header → page actions → toolbar →
   refinements → content) and the controls-and-editing rules in §10
   (read-only values and quick actions on the page; free-form edits in a
   dialog; help behind an info button).
2. The bottom navigation becomes a three-zone bar (§6): leading browser-style
   back/forward over *locations* plus the current location's name; the
   landmark capsule in the centre; the current instance on the trailing edge,
   switchable from a popover. Pages no longer draw their own back buttons.
3. `ARCH.md` gains two top-level nodes, **Accounts** and **Settings**, as
   landmarks. Their pages are built by their own plans; until then they are
   not in the capsule.

## Consequences

- Positive: one place to look for each kind of control; the install target
  stops crowding Discover; adding Accounts/Settings later needs no new
  navigation pattern.
- Negative / trade-offs: the shell now owns a location history and must drop
  entries whose subject disappears (a deleted instance); dialogs need the
  framework `Root`, so UI tests for them mount one.
- Follow-ups: persist the current instance in core settings and make Home's
  Continue honour it (today it is UI state seeded from the most recently
  played instance); migrate inline two-click delete confirmations to alert
  dialogs; build Accounts and Settings pages.
