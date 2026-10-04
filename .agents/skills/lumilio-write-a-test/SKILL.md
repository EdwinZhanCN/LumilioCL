---
name: lumilio-write-a-test
description: Use when adding a LumilioCL test — choose the core, UI, or app
  boundary, keep domain tests independent of GPUI, and prove the guard can fail.
---

# Write A Test In The Right Layer

Choose the layer from the behavior under test, not from the file being edited.
The project boundary is fixed: `lumilio-core` is reusable domain logic,
`lumilio-ui` owns GPUI behavior, and `lumilio-app` owns process assembly.

## Placement

| Behavior | Test home | Rule |
| --- | --- | --- |
| Pure domain transformation, validation, parsing, or state transition | `crates/lumilio-core` unit tests or its integration tests | No `gpui` or `gpui-component` dependency; test inputs/outputs and edge cases |
| View model, page state, component interaction, or rendering behavior | `crates/lumilio-ui` | Load the `gpui` and `gpui-component` skills; use existing component/test patterns |
| Startup wiring, command-line behavior, or cross-crate assembly | `crates/lumilio-app` integration/smoke tests | Exercise the shipped entry path rather than a hand-built substitute |

For UI interaction, prefer pure state tests first (e.g. `HomePresentation`
transitions). When the behaviour depends on the real element tree — a click
reaching a handler, a state actually drawing — use a `#[gpui::test]` with
`add_window_view`, mark the target with `.debug_selector(..)` (compiled out of
release builds), and drive it with `debug_bounds` + `simulate_click`. The
reference is `pressing_continue_plays_the_launch_moment_through_every_state`
in `crates/lumilio-ui/src/shell.rs`.

Keep tests beside the layer they characterize. If a UI test needs complex
domain setup, extract and test that logic in core instead of making core know
about GPUI.

## Prove the guard

For a regression test:

1. Introduce the smallest version of the regression.
2. Run the focused test and observe it fail for the intended reason.
3. Revert the regression and keep the test.
4. Run the focused test again, then the full four-command loop.

Assert externally observable behavior. Do not make a test pass by checking an
internal self-report, a log keyword, or an unverified mock call.

## Verification

During editing, run the narrowest focused test (`just test-pkg <crate> <filter>`).
Before handoff, run `just check`.

If the workspace is not yet bootstrapped, record the exact missing-artifact
diagnostic in the active plan instead of inventing a substitute gate.

When an API takes a generic source iterator, use explicit String values (for example `to_owned()`) rather than ambiguous `.into()` inside `vec!`. Treat inference/compile failures as setup failures, never regression evidence.
