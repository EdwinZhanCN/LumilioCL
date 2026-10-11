# XaeroTools vendored source snapshot

Baseline: `dekrom/xaerotools` commit `66694bf5a3d13ba55bd35d9045832353063422eb`.
License: MIT (`LICENSE`). This is a subset of the unmodified upstream files:
`crates/xaero-core`, its `crates/test-support` dev dependency, and the two
embedded assets needed by `xaero-core`. The root `Cargo.toml` only narrows the
workspace to those crates. LumilioCL uses the region decoder and data model;
it does not maintain a XaeroTools fork. Any future source patch must be
recorded here and guarded by a test before this baseline moves.
