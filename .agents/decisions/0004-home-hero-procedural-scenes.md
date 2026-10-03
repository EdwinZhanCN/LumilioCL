# 0004 — Home hero is a procedural, mechanic-driven pixel carousel

- Status: accepted
- Date: 2026-09-29

> Numbering note: `crates/lumilio-ui/src/assets.rs` cites ADR 0003, but ADRs
> 0001–0003 are not present in this tree. This record starts at 0004 so it
> never collides with those references.

## Context

Home (ARCH node `Home`) starts in the visual-only `Ambient` state before domain
data exists, and previously reserved an empty 280 px box. An earlier ASCII
backdrop (`lumilio-ui::backdrop`) was rejected and is kept only as a test-only
experiment. The launcher needs a first impression that feels unmistakably
Minecraft without shipping game assets, copying `3rd-party/`, or pulling in a
new rendering dependency.

## Decision

Home carries a hero carousel of procedural pixel-art scenes painted with GPUI's
`canvas` + `paint_quad`. Each scene animates a real game mechanic (sunrise,
block light decay, redstone signal decay, portal ignition) and slides change via
a spiral "chunk loading" transition. The hero is a presentation layer of the
`Home` node: it introduces no route, navigation item, or domain contract.

Rules that come with it:

- No bitmap or third-party art; every texel is computed from original code.
- Scene rasterisation is pure Rust with no GPUI types, so it is unit-tested.
- The hero only requests animation frames while it is rendered (Home route)
  and never when `App::reduce_motion` is set; in that mode it shows a still
  composition and does not auto-advance.
- No new crate dependencies.

## Consequences

- Positive: a distinctive, on-brand first screen; zero asset/licensing risk;
  mechanics are testable (light and signal decay are asserted in tests).
- Negative / trade-offs: CPU rasterisation per frame while Home is visible;
  bounded by a capped texel grid and run-length merged quads.
- Follow-ups: a later plan may feed domain data (last played world) into the
  hero copy, or move rasterisation to a shader if GPUI exposes one.
