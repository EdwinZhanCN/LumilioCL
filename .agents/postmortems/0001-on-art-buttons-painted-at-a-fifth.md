# Postmortem 0001: On-art buttons painted at a fifth of their colour

## Executive summary

Buttons placed on Home's world art (**继续**, **恢复并继续**, **结束游戏**,
**取消**, **技术详情**) were styled with gpui-component's custom button variant.
That variant paints its resting background at 20 % of the given alpha and
draws no border outside outline mode, so the "white" primary rested at 20 %
white and the dark glass secondary was nearly invisible, borderless, and its
hover (a slightly darker black) did not read over the dimmed world. The
maintainer spotted it on **结束游戏**.

## What broke

`ButtonCustomVariant` was assumed to paint `color` as the resting fill. In
gpui-component 0.7 the resting fill is `color.mix_oklab(transparent, 0.2)`
and border edges are only enabled for the default variant or outline mode.

## Why every net missed it

- The contact sheet only covers world art, not GPUI chrome.
- The window test proved buttons render and are clickable, not what they
  paint.
- No screenshot of the live window was available in the agent session, and
  the assumption about the variant was not checked against its source.

## Guardrails added

- Regression test `buttons_on_the_world_have_a_visible_rest_state_border_and_hover`
  in `crates/lumilio-ui/src/shell.rs` asserts on `Window::painted_quads()`:
  opaque white primary at rest, visible glass fill and border, and a hover
  that changes the fill. Reverting the fix fails it (primary reported at
  alpha 0.2).
- `ArtStyle` in `crates/lumilio-ui/src/home.rs` sets the resting fill and
  border explicitly and documents the variant's behaviour.
- `.agents/skills/lumilio-motion-design/SKILL.md` §5 now requires asserting
  painted colours (not just bounds) for styled chrome, and warns about the
  custom variant.
