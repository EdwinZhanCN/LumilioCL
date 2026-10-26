---
name: lumilio-motion-design
description: Use for LumilioCL layout, interaction, animation or world-scene changes. Apply relevant design guidance and review the changed behavior; copy-only work uses lumilio-i18n.
---

# Interface And Motion Review

Start from the feature's task and nearest implementation. Read the relevant
sections of `docs/design-language.md` and `docs/design-patterns.md`, plus GPUI
skill references for the APIs and interactions involved. These project documents
give defaults, not a required page structure. Adapt them to the feature without
a separate approval or a rule change for every local choice.

## Implement the changed behavior

- Reuse the project's controls and semantic theme tokens. Check APIs against
  the locked dependency and local helper signatures.
- Keep state and feedback truthful. Async edits retain the initiating entity
  and target; navigation must not redirect their results. Preserve unrelated
  drafts when saving one group of fields.
- For animation, start with `theme::motion`, honor reduced motion, and stop
  decorative tickers when hidden or while the game runs. Keep I/O off the UI thread.
- For changed text, use `lumilio-i18n` and `docs/plain-language.md`.
  Wording and presentation can change together.

## Review what changed

Choose checks for the affected behavior, not every item for every edit:

- Interface layout or controls: run with a disposable data directory
  (`LUMILIO_HOME=<temp folder> cargo run -p lumilio-app`). Review relevant
  interaction states, keyboard/focus behavior, both appearances, and narrow
  windows or long translations where layout can change.
- World rendering: produce and inspect frames with
  `LUMILIO_HERO_DUMP=<dir> cargo nextest run -p lumilio-ui hero_contact_sheet --ignored`.
- Continuous animation: compare CPU use while visible and idle; ensure it
  does not compete with the game.
- A regression worth guarding: use `lumilio-write-a-test`; existing GPUI tests
  demonstrate `debug_bounds`, `simulate_click` and `painted_quads`.

Use `lumilio-select-checks` for handoff. State what was checked and what still
needs visual review. Guidance-only edits do not require launching the app.
Update the design references only when a reusable direction changes; record
important feature-specific tradeoffs in the existing plan when useful.
