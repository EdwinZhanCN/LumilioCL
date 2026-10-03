---
name: lumilio-motion-design
description: Use for every LumilioCL change that adds or alters UI, animation,
  copy, or a world-register scene — classify the work against the design
  language, pick motion tokens, meet the performance and reduced-motion rules,
  and review the result visually before handoff.
---

# Motion And Interface Design

`docs/design-language.md` is the specification; this skill is the procedure
that applies it. Load the `gpui` and `gpui-component` skills as well — they
cover the APIs, this covers the decisions.

## 1. Classify the change

Decide which register every new element belongs to (design language §1):

- **World** — procedural pixel art inside `lumilio-ui::hero` or a future
  cover/vignette renderer. Discrete motion at `theme::motion::WORLD_TICK`,
  stepped corners, no text inside the art.
- **Interface** — gpui-component elements styled from theme tokens, eased with
  `theme::motion` durations.

An element that needs both is split: world art underneath, interface
foreground on top (the Home hero overlay is the reference pattern).

## 2. Choose motion from the tokens

- Use `theme::motion::{INSTANT, QUICK, SETTLE, SCENE}`; never a literal
  `Duration` in UI code.
- Entrances: `ease_out_quint`, travel `LIFT`. Exits: shorter, no bounce.
- Key animations by the state they express (`("launch-bar", subject_id)`), so
  a new state restarts them and an unchanged state does not.
- Progress displays must be monotonic and ease toward the reported value.

## 3. Meet the runtime rules

- `with_animation` already respects `App::reduce_motion`; custom tickers and
  scene clocks must check it and fall back to a still composition.
- A world animation ticks only while rendered (the `HeroCarousel` ticker with
  its visibility grace window is the reference) and stops entirely while the
  game is running.
- Paint large texel grids through `PixelGrid::rects` inside one
  `Window::paint_layer`.
- No blocking I/O in render or paint; state arrives through entity updates.

## 4. Write the copy

Follow design language §8: calm Simplified Chinese, one primary action,
details behind **技术详情**, no raw error strings as headlines. Domain types in
`lumilio-core` never carry display strings; the UI maps them to copy.

## 5. Test and review

- Pure logic (state transitions, progress math, scene mechanics) gets unit
  tests without a window.
- Check both appearances. World frames come out as `-dark` and `-light`
  variants from the contact sheet; interface changes must use theme tokens
  (or the on-art button styles) rather than fixed colours.
- Layout regressions are testable: give the element a `.debug_selector(..)`
  and assert its `debug_bounds` in a `#[gpui::test]` (see the Continue width
  assertion in `shell.rs`).
- Styled chrome is testable too: `Window::painted_quads()` (gpui
  `test-support`) returns what was painted. Assert colours, borders, and hover
  changes, not only bounds — a fill and its border arrive as separate quads.
  Wait for real-time entrance animations before measuring (see `settle` in
  `shell.rs`).
- Do not trust a component's styling API by its name: gpui-component's
  `ButtonCustomVariant` rests at 20 % of its colour and has no border outside
  outline mode (postmortem 0001). Read the component source or assert the
  painted result.
- World-register changes: render frames with
  `LUMILIO_HERO_DUMP=<dir> cargo test -p lumilio-ui hero_contact_sheet -- --ignored`
  and look at them (`sips -s format png` converts PPM on macOS).
- Interface changes: run `LUMILIO_HOME=<temp folder> cargo run -p lumilio-app` (a
  disposable data folder; review hooks are in `README.md`) and check hover, focus,
  reduced motion, and the 720×480 minimum window.
- When a change animates continuously, measure CPU time while it is visible
  (`ps -o utime= -p <pid>` over a few seconds) against the §4 budget.
- Finish with the four verification commands from `AGENTS.md`.

## 6. Keep the spec honest

If the change needs a rule the design language does not have, or breaks one on
purpose, update `docs/design-language.md` in the same change and record why.

## Live forms and pinned APIs

- Before composing a component, inspect the pinned dependency's module exports,
  required traits, and callback arity, plus local kit helper signatures. Do not
  assume online examples match the locked version.
- Bind asynchronous reads and saves to the initiating entity and stable target
  ID. A route change must not redirect the operation or its result.
- When fields save independently, synchronize only the successfully saved group;
  preserve unrelated unsaved drafts. Cover failure, repeat submit, and stale-target
  responses with interaction tests.

For native macOS review, use a task-owned temporary `.app` bundle when the
computer-use inventory cannot resolve a bare executable. Restrict temporary-file
searches to that task directory; do not scan the whole system temp tree. Verify
both appearance hooks survive the window-appearance callback, and verify the
application's standard quit action before recording a restart check as passed.
