# 0043 — Run checks on Linux and keep platform builds in release

- Status: accepted
- Date: 2026-10-10

## Context

CI ran the full `just ci` check on macOS and also had a separate Windows job that only built `lumilio-cubiomes`. Release packaging already builds complete artifacts on native macOS, Windows and Linux runners.

## Decision

Run the single `just ci` job on `ubuntu-latest` and remove the standalone Windows cubiomes job. Keep platform-specific package builds in the release workflow's native runner matrix.

## Consequences

- Positive: routine CI has one platform-neutral check path, while release remains responsible for proving each platform's complete package build.
- Negative / trade-offs: Windows-only build failures are found in release workflow runs rather than a dedicated per-push cubiomes job.
- Follow-ups: use `workflow_dispatch` on the release workflow to rehearse the native package matrix before publishing 0.1.0.

Shipped: `.github/workflows/ci.yml` now runs `just ci` on Linux and no longer builds cubiomes separately on Windows; `.github/workflows/release.yml` is unchanged.
