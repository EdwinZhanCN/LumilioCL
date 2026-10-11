# Every check, locally and in CI, runs through these recipes. CI runs `just ci`
# (.github/workflows/ci.yml); nothing else defines what "passing" means.

set shell := ["bash", "-euo", "pipefail", "-c"]

# List the recipes.
default:
    @just --list

# The full loop, in order. Run it before handing off a code change.
check: prune build test clippy fmt

# Cargo never deletes stale artifacts from target/. Past `limit` GB, clean it:
# one full rebuild is cheaper than every rustc call scanning a huge deps dir.
prune limit="80":
    #!/usr/bin/env bash
    set -euo pipefail
    [ -d target ] || exit 0
    kb=$(du -sk target | cut -f1)
    if [ "$kb" -gt $(( {{limit}} * 1024 * 1024 )) ]; then
        echo "target/ is $(( kb / 1024 / 1024 )) GB (limit {{limit}} GB); cleaning"
        cargo clean
    fi

# What CI runs on every push and pull request.
ci: check

# Compile the workspace.
build:
    cargo build

# Every test in the workspace: nextest, then doctests (nextest skips them).
test:
    cargo nextest run
    cargo test --doc

# Lints, with warnings as errors.
clippy:
    cargo clippy --all-targets -- -D warnings

# Fail if anything is not rustfmt-formatted.
fmt:
    cargo fmt --check

# Format everything in place.
format:
    cargo fmt

# Tests of one crate while iterating, e.g. `just test-pkg lumilio-ui project_detail`.
test-pkg pkg *filter:
    cargo nextest run -p {{pkg}} {{filter}}

# Regenerate docs/ia/paths from the `// ia[...]` comments.
ia:
    cargo run -p lumilio-docgen -- ia

# Generate the retained plans and explicitly public roadmap projection.
plans:
    cargo run -p lumilio-docgen -- plans generate

plans-check:
    cargo run -p lumilio-docgen -- plans check

# Rewrite the list of Chinese string literals still in UI and app code
# (crates/lumilio-ui/src/i18n/hardcoded.txt), which may only shrink, and run
# the other catalog tests (parsing, ids and arguments) with it.
hardcoded-chinese:
    LUMILIO_BLESS_HARDCODED=1 cargo nextest run -p lumilio-ui --lib i18n::

# Release packages for this platform into dist/ (assets/icons/PACKAGING.md).
package:
    cargo xtask package

# Print the release version; with a tag, fail unless it is v<version>.
release-check *tag:
    cargo xtask release-check {{tag}}

# The website and release mirror in web/ (pnpm): types, Worker tests, build.
web:
    cd web && pnpm install --frozen-lockfile
    cd web && pnpm check
    cd web && pnpm test
    cd web && pnpm build

# Cheap check for docs/harness changes: IA is current, attributions are right.
docs:
    cargo nextest run -p lumilio-docgen
    # Workspace feature unification must not change generated JSON bytes.
    cargo nextest run -p lumilio-docgen --lib --features serde_json/preserve_order plans::
    just plans-check
    cargo fmt --check
