# XaeroTools vendored source snapshot

Baseline: `dekrom/xaerotools` commit `7bc650bdf445ec06d0d3fc0fb9e98ba4b86a6b83`.
License: MIT (`LICENSE`). This is a subset of the unmodified upstream files:
`crates/xaero-core`, its `crates/test-support` dev dependency, and the two
embedded assets needed by `xaero-core`. The root `Cargo.toml` only narrows the
workspace to those crates. LumilioCL uses the region decoder and data model;
it does not maintain a XaeroTools fork. Any future source patch must be
recorded here and guarded by a test before this baseline moves.
