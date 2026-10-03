# Postmortem 0004: Changing a game's version crashed the app

## Executive summary

Pressing 更换 in the change-version dialog aborted the process with "cannot update NewGameForm while it is already being updated". `NewGameForm::submit` now defers the caller's handler until the click has finished updating the form, and a test drives a handler that finishes the form from inside it.

## What broke

`submit` called the handler while the form's own click update was still running. The change flow's handler answers `Create` by calling `form.created(...)` on the same form, which gpui forbids. The panic happened inside a platform callback that cannot unwind, so the whole app aborted instead of showing an error. Creating a new game was unaffected because its handler finishes the form later, from an async task.

## Why every net missed it

The change-runtime test used a handler that only recorded intents, so the one path where the handler re-enters the form was never exercised. The core tests covered `change_runtime` end to end but not the dialog's wiring in `lumilio-app`. Plan 0030 said the real change had not been clicked and asked for a manual pass; that pass found it.

## Guardrails added

- [Regression test](../../crates/lumilio-ui/src/new_game.rs) `a_handler_may_finish_the_form_it_was_called_from`: a change dialog whose handler calls `created` on the form from inside the click. It fails with the original panic when the deferral in `submit` is removed.
- The fix is in `submit` rather than in the one handler, so any current or future handler can answer by updating the form.
