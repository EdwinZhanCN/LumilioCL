# 0005 — Launch progress is a core contract; Home renders it

- Status: accepted
- Date: 2026-09-30

## Context

Home's launch moment (design language §5) needs stages, item counts, and a
progress value it can animate without lying. Core already emits
`InstallEvent`s and has repair verification, but no process launch service
exists yet, and core must not know about GPUI or display copy.

## Decision

1. `lumilio-core::launch_session` owns the launch progress contract:
   `LaunchPhase` (Verifying, Libraries, Assets, Starting), `LaunchSignal`
   inputs, and a `LaunchSession` reducer that guarantees forward-only phases
   and monotonic progress, and reports terminal outcomes (`Running`,
   `Exited`, `Failed` with a typed `LaunchFailure`). It carries no display
   strings. `InstallEvent`s map onto signals.
2. The UI maps the session to Home presentations and copy; the hero's
   chunk-loading reveal is driven by the session's progress.
3. Until a real launch service lands, `lumilio-app` contains a preview driver
   that emits scripted signals, active only when `LUMILIO_PREVIEW` is set.
   (Removed by plan 0036: the real launch service now feeds `LaunchSession`.)

## Consequences

- Positive: the launch UI is built and tested against the exact interface the
  real service will implement; progress honesty rules live in one tested
  place.
- Negative / trade-offs: a scripted driver exists in the app binary; it is
  env-gated and documented as a preview. `lumilio-ui` gains a dev-only
  dependency on gpui's `test-support` feature for window-level UI tests.
- Follow-ups: a launch-service plan feeds `LaunchSession` from repair,
  install, and process supervision, and removes the need for the preview in
  day-to-day development.
