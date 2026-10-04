# 0026 — Checks live in the justfile; CI runs them; no git hooks

- Status: accepted
- Date: 2026-10-04

## Context

A `pre-commit` hook ran the full loop (build, test, clippy, fmt) on every commit. A full run takes
several minutes, and much longer after a change that rebuilds the workspace. Commits stalled behind
it, including commits that only touched documents. The same four commands were also spelled out in
AGENTS.md, two skills and the hook, so there were several places that could drift apart.

## Decision

- The `justfile` is the single source of truth for checks. `just check` is the full loop (build →
  test → clippy → fmt); `just docs` is the quick check for documentation and harness changes
  (docgen tests and rustfmt); `just test-pkg`, `just ia` and the others are for iterating.
- GitHub Actions (`.github/workflows/ci.yml`) runs `just ci` on macOS for every push to `main` and
  every pull request. Pushes that change only `.agents/**` or Markdown are skipped, except for the
  generated `docs/ia/**`.
- There are no git hooks. A code change is still handed off only after `just check`; CI is a
  backstop, not the evidence for handoff.

## Consequences

- Positive: committing never waits on a hook, documents cost seconds to check, and CI cannot
  disagree with local checks because both call the same recipe.
- Negative / trade-offs: a commit can land without passing the loop if someone skips
  `just check`. CI then reports it after the push instead of blocking it. Contributors need `just`
  installed.
- Supersedes the hook described in the commit "Run the full verification loop at commit time"
  (`46b7160`).
