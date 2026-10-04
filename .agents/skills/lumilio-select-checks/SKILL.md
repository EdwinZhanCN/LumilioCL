---
name: lumilio-select-checks
description: Use before claiming a LumilioCL change is complete — map the diff
  to focused Cargo checks while preserving the repository's mandatory four-step
  verification loop.
---

# Select Checks For LumilioCL

Use the narrowest check during iteration, then run the complete verification
loop required by `AGENTS.md` before handoff. This avoids wasting time while
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

| Diff area | Focused iteration check | Required completion loop |
| --- | --- | --- |
| `crates/lumilio-core/**` | `cargo test -p lumilio-core` | `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` |
| `crates/lumilio-ui/**` | `cargo check -p lumilio-ui` or its focused test | The same four commands; load `gpui`/`gpui-component` skills for UI behavior |
| `crates/lumilio-ui/src/hero/**` | `cargo test -p lumilio-ui hero::`, then review frames from `LUMILIO_HERO_DUMP=<dir> cargo test -p lumilio-ui hero_contact_sheet -- --ignored` | The same four commands; scene changes are visual, so look at the frames before handoff |
| `crates/lumilio-app/**` | `cargo check -p lumilio-app` | The same four commands |
| workspace manifests/toolchain | `cargo metadata --no-deps` | The same four commands |
| `docs/**`, `AGENTS.md`, `.agents/**` | link/path review and `cargo fmt --check` if no code changed | The same four commands, unless the environment cannot yet build the skeleton |
| `3rd-party/**` | none — the tree is read-only | Do not modify, move, or delete it |

Focused checks are not a substitute for the final loop. Run the four commands
in this exact order, fix failures immediately, and report only commands that
actually ran. Explicitly name checks skipped because the repository is still a
skeleton or the environment lacks a required toolchain.

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
