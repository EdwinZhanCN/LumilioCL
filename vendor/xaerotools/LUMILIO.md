# XaeroTools vendored source snapshot

Baseline: `dekrom/xaerotools` commit `66694bf5a3d13ba55bd35d9045832353063422eb`.
License: MIT (`LICENSE`). This is a subset of the upstream files, unmodified
except for the patches below: `crates/xaero-core`, its `crates/test-support`
dev dependency, and the two embedded assets needed by `xaero-core`. The root
`Cargo.toml` only narrows the workspace to those crates. LumilioCL uses the
region decoder and data model; it does not maintain a XaeroTools fork. Any
future source patch must be recorded here and guarded by a test before this
baseline moves.

## Local patches

- zip 9 (`crates/xaero-core/Cargo.toml`, `crates/xaero-core/src/codec/zipio.rs`).
  Upstream requires zip 8.6.0, which crates.io yanked on 2026-10-16. zip 9's
  entry `name()` returns a `Result`; `read_region_container` treats an
  undecodable name as not `region.xaero`. Guard:
  `lumilio-plugin-world-explorer`
  `a_region_container_reads_the_region_entry_after_another_one`.
