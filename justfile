# Every check, locally and in CI, runs through these recipes. CI runs `just ci`
# (.github/workflows/ci.yml); nothing else defines what "passing" means.

set shell := ["bash", "-euo", "pipefail", "-c"]

# List the recipes.
default:
    @just --list

# The full loop, in order. Run it before handing off a code change.
check: build test clippy fmt

# What CI runs on every push and pull request.
ci: check

# Compile the workspace.
build:
    cargo build

# Every test in the workspace.
test:
    cargo test

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
    cargo test -p {{pkg}} {{filter}}

# Regenerate docs/ia/paths from the `// ia[...]` comments.
ia:
    cargo run -p lumilio-docgen -- ia

# Rewrite the list of Chinese string literals still in UI and app code
# (crates/lumilio-ui/src/i18n/hardcoded.txt), which may only shrink, and run
# the other catalog tests (parsing, ids and arguments) with it.
hardcoded-chinese:
    LUMILIO_BLESS_HARDCODED=1 cargo test -p lumilio-ui --lib i18n::

# Release packages for this platform into dist/ (assets/icons/PACKAGING.md).
package:
    cargo xtask package

# Print the release version; with a tag, fail unless it is v<version>.
release-check *tag:
    cargo xtask release-check {{tag}}

# The website and release mirror in web/ (pnpm): types, Worker tests, build.
web:
    pnpm --dir web install --frozen-lockfile
    pnpm --dir web check
    pnpm --dir web test
    pnpm --dir web build

# Cheap check for docs/harness changes: IA is current, attributions are right.
docs:
    cargo test -p lumilio-docgen
    cargo fmt --check
