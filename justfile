# Every check, locally and in CI, runs through these recipes. CI runs `just ci`
# (.github/workflows/ci.yml); nothing else defines what "passing" means.

set shell := ["bash", "-euo", "pipefail", "-c"]

# List the recipes.
default:
    @just --list

# The full loop, in order. Run it before handing off a code change.
check: prune build test clippy fmt rustdoc

# Cargo never deletes stale artifacts from target/. Past `limit` GB, clean it:
# one full rebuild is cheaper than every rustc call scanning a huge deps dir.
prune limit="80":
    #!/usr/bin/env bash
    set -euo pipefail
    [ -d target ] || exit 0
    kb=$(du -sk target | cut -f1)
    if [ "$kb" -gt $(( {{limit}} * 1024 * 1024 )) ]; then
        echo "target/ is $(( kb / 1024 / 1024 )) GB (limit {{limit}} GB); cleaning"
        # target/block-assets is a bake and its downloaded jars, not build
        # output (cargo xtask bake-blocks); it survives the clean.
        if [ -d target/block-assets ]; then mv target/block-assets .prune-block-assets; fi
        status=0
        cargo clean || status=$?
        if [ -d .prune-block-assets ]; then mkdir -p target && mv .prune-block-assets target/block-assets; fi
        exit "$status"
    fi

# What CI runs on every push and pull request: the full loop and the forks' tests.
ci: check forks

# The forks under forks/ are outside the workspace, so `test` never builds
# them; each runs in its own target directory so a workspace build cannot
# clobber it. Tests that read the baked block bundle say so and pass without
# it; `cargo xtask bake-blocks` runs the entity coverage check on its bundle.
# The tests of the maintained forks the launcher builds from.
forks:
    CARGO_TARGET_DIR=target/forks/schematic-mesher cargo test --manifest-path forks/schematic-mesher/Cargo.toml
    CARGO_TARGET_DIR=target/forks/nucleation cargo test --manifest-path forks/nucleation/Cargo.toml -p nucleation --no-default-features --features meshing,rendering --lib

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

# Check API links, including internal modules moved behind public facades.
rustdoc:
    RUSTDOCFLAGS="${RUSTDOCFLAGS:-} -D rustdoc::broken_intra_doc_links" cargo doc --workspace --no-deps --document-private-items

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

# Rewrite web/src/data/themes.generated.json (the theme studio's built-in
# themes and contrast rules) from the launcher's catalog.
website-themes:
    LUMILIO_BLESS_THEMES=1 cargo nextest run -p lumilio-ui --lib theme::tests::website

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
