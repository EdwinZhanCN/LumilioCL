---
name: lumilio-select-checks
description: Use before claiming a LumilioCL change is complete — map the diff
  to the narrowest justfile recipe while iterating, then `just check` (code) or
  `just docs` (docs only) before handoff.
---

# Select Checks For LumilioCL

Use the narrowest check during iteration, then the handoff recipe required by
`AGENTS.md`. This avoids wasting time while
editing without weakening the final evidence.

## Inspect the scope

```sh
git status --short --branch
git diff --stat
```

If Git metadata is unavailable, inspect the changed paths directly and record
that limitation. Existing dirty files are user-owned; do not reset or reformat
them to manufacture a clean baseline.

## Evidence map

Every command here is a `justfile` recipe, the single source of truth that CI also runs.

| Diff area | While iterating | Before handoff |
| --- | --- | --- |
| `crates/lumilio-core/**` | `just test-pkg lumilio-core [filter]` | `just check` |
| `crates/lumilio-ui/**` | `just test-pkg lumilio-ui [filter]`; load repo-local `gpui-kit`/`gpui-kit-design-guides` skills for UI behavior | `just check` |
| `crates/lumilio-ui/src/hero/**` | `just test-pkg lumilio-ui hero::`, then review frames from `LUMILIO_HERO_DUMP=<dir> cargo test -p lumilio-ui hero_contact_sheet -- --ignored` | `just check`; scene changes are visual, so look at the frames |
| `crates/lumilio-app/**` | `cargo check -p lumilio-app` | `just check` |
| workspace manifests/toolchain, `justfile`, `.github/**` | `cargo metadata --no-deps` | `just check` |
| `docs/**`, `AGENTS.md`, `.agents/**`, `*.md` only | link/path review | `just docs` (seconds); no full loop |
| `.agents/plans/*.json`, `.agents/schemas/**`, `docs/plans/**` | `just plans`, then `just plans-check` | `just docs`; `just web` if public roadmap changes |
| `web/**` | `just web` | `just web`; generated roadmap changes also require `just docs` |

Focused checks are not a substitute for `just check` on a code change. Report only
recipes that actually ran, and name anything skipped. CI repeats `just ci` after push;
it is a backstop, not the evidence for handoff.

## Handle failures

A relevant failure blocks handoff. Fix it or preserve the exact diagnostic as
an open question in the active plan; never claim that CI will resolve a local
failure later. If the failure reveals a recurring agent mistake, add a rule,
skill checklist, test, or command in the same change.

## Scripted documentation edits

When using Python to edit prose, use triple-quoted strings for multiline text
and text containing apostrophes; a syntax error means no edit has run. Check the
command exit status and inspect the affected document before recording completion.

## New test paths

Before writing a new test file with a heredoc, verify or create its parent directory. A red test is evidence only when compilation completed and the intended behavioral assertion failed; missing-directory shell errors are setup failures.
