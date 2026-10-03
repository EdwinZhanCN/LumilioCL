# Postmortem 0003: gpui-component's own icons never loaded

## Executive summary

Every icon gpui-component draws by itself — the checkbox tick, the dialog ✕,
select chevrons, the notification type icons — rendered blank in the real app.
Checked checkboxes looked like plain white squares and dialogs had no close
button. Found during the new-game dialog's screenshot review (plan 0024).

## What broke

The workspace depends on `gpui-kit` with `default-features = false,
features = ["component"]`, which drops the `assets` feature, and the app
registered only `lumilio_ui::assets::Assets`. That source returns `None` for
every path it does not carry, and nothing fell back to the component bundle.

## Why every net missed it

- UI tests run under GPUI's test platform, which never loads SVGs, so a
  missing icon is invisible to them.
- Our own icons (`UiIcon`, `LandmarkIcon`) are covered by
  `every_declared_icon_path_loads`; component icons were assumed to be
  provided by the library.
- Visual review had been done mostly on pages without component icons, and
  the locked screen prevented agent screenshots in earlier rounds.

## Guardrails added

- `crates/lumilio-app/src/assets.rs` composes our source with
  `gpui_kit::assets::Assets`; the workspace enables gpui-kit's `assets` feature.
- Regression test `assets::tests::component_icons_and_our_own_both_load`
  (lumilio-app) loads component icon paths through the registered source.
