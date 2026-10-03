# 0013 — The interface is an instrument: a hardware control language and brand type

- Status: accepted
- Date: 2026-10-03

## Context

The interface register was stock gpui-component (a shadcn/ui look): correct,
but with nothing of its own, while the world register (procedural pixel art)
carries all of LumilioCL's identity. The maintainer asked for a control style
with character, keeping every layout as it is and changing only the controls.

Three directions were built side by side in a style lab
(`crates/lumilio-app/examples/style_lab.rs`, since removed) and reviewed as screenshots in
both appearances: a pixel-notch style borrowed from the world, a generic
"warm hardware" style, and a blend of the two. The maintainer rejected all
three as not specific enough. They then supplied reference photographs of
teenage engineering products and print (TP-7, K.O. II, the EP series, the
website). A language extracted from those was built in the same lab, and the
maintainer accepted it, with one change: Chinese text stays on the system
font, because bundling a CJK face (about 8 MB per weight) is not worth it.

Facts that constrain the implementation:

- GPUI can register fonts from memory (`TextSystem::add_fonts`) and resolves
  glyphs missing from a family through the platform's fallback. In the lab on
  macOS, Chinese under Space Grotesk fell back to PingFang, including its
  light weight.
- gpui-component's stock components (Dialog, Popover, menus, Notification,
  Input, Select) follow theme tokens, but their shapes can only be changed by
  wrapping or replacing them. `ButtonCustomVariant` rests at 20 % of its
  colour (postmortem 0001), so keys cannot be built on it.
- White text on the signal orange measures about 3.5:1 (aluminium body) and
  3.1:1 (night body). That is enough for large or bold text, but below 4.5:1
  for 14 px labels.

## Decision

1. **The interface is drawn as an instrument.** `docs/design-language.md`
   §12 defines the control language. Its rules:
   - One neutral body (aluminium in light, night in dark) and one signal
     orange.
   - Buttons are physical keys: white, black and orange.
   - State is shown by LEDs.
   - Tabs are port labels.
   - Groups are marked with silkscreen: a numbered label and a bracket
     hairline.
   - Live numbers sit on a black display; key/value lists are hairline tables.
   - Instance cards are faceplates whose cover is the display.

   The world register is unchanged. The interface still never imitates game
   GUI textures.
2. **Brand type, system Chinese** (§13):
   - Space Grotesk for the interface's Latin letters and figures.
   - JetBrains Mono for values.
   - DSEG7 Classic only on displays.
   - All three are embedded in `lumilio-ui` under their OFL licences and
     registered from memory at start-up.
   - Chinese is not bundled. It resolves to the platform's system font
     through fallback.
3. The migration is plan 0031. Controls are restyled at the `kit` layer and
   the theme-token layer, so that page code keeps its structure.

## Consequences

- Positive:
  - The interface gets an identity that sits next to the world art without
    competing with it. Both are "objects you look into or press".
  - Restyling through `kit` and theme tokens touches pages only where they
    call gpui-component directly.
  - About 1 MB of fonts instead of about 33 MB.
- Negative / trade-offs:
  - Keys, LEDs, faders and port tabs become our own elements instead of stock
    components, so they need their own focus, hover and press tests.
  - Chinese glyph shapes differ per platform (PingFang, Microsoft YaHei,
    Noto/Source Han on Linux), and the light title weight depends on what the
    platform provides.
  - Latin silkscreen captions bend the "Simplified Chinese copy" rule. §8 now
    limits them to echoes, proper nouns, numbers and units.
- Follow-ups (owned by plan 0031):
  - Text contrast on orange: either darken the orange or move key labels to
    ink, decided at visual acceptance.
  - Choose a danger colour that cannot be confused with the signal orange.
  - Verify the Chinese fallback and light weight on Windows and Linux. Add an
    explicit fallback list if a platform picks a poor face.
  - Remove the style lab when the migration is done (done).
