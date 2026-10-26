# LumilioCL Design Language

Design defaults for a native Minecraft launcher. Read the sections relevant to
the task. Layout, density, controls and motion serve the feature: adapt these
defaults when they make the work harder to understand or complete. A local
adaptation needs neither a documentation change nor separate approval.

This is not an inventory of the current interface or a specification for every
page. Use the code and [user paths](ia/paths/README.md) for current behavior,
[patterns](design-patterns.md) for recurring interactions, and
[plain language](plain-language.md) for writing. Update this reference when a
useful direction changes across the product; keep feature plans in plans.

## 1. World and interface

The game's world gives LumilioCL its character: pixel scenes, covers and
landmarks. The interface is a calm instrument around it, with readable text,
physical keys, small indicators and clear values. Discrete world motion and
smooth interface feedback are a useful contrast, not a requirement to animate
every feature. Art should leave room for the task and never obscure controls.

## 2. Motion tokens

Start with `theme::motion` in `crates/lumilio-ui/src/theme/mod.rs`:
`INSTANT` for direct feedback, `QUICK` for small changes, `SETTLE` for larger
transitions, `SCENE` for world entrances, and `WORLD_TICK` for world animation.
The code owns their values. Reuse these tokens before adding a new timing.

## 3. Motion grammar

Use motion to show a change of state or preserve spatial continuity. Immediate
updates are often better for repeated editing. Progress reflects real work;
animation does not invent progress or hide failure. Honor reduced motion and
make the same state understandable without animation.

## 4. Performance is part of the experience

Decorative work should stop when hidden and while the game is running.
Continuous animation deserves a CPU check; it should not compete with play.
Keep I/O off the UI thread and use existing rendering paths for dense scenes.

## 5. Home

Home should help the player start or resume a game and notice what prevents
that. A world scene can express the current game's state, but a plain surface
is also useful. Give actions and recovery information the space they need;
there is no required hero height, row count or arrangement here.

## 6. Navigation

Keep location, selected objects and the way back understandable. Reuse the
shell's navigation and shared choices. A specialized workspace may need its
own navigation or window. A change of page or selection must not silently
retarget an operation that has already started.

## 7. Page anatomy

A title, relevant actions, search or filters, and content are useful starting
points. Combine, omit or rearrange them to fit the task. Editors, maps and
multi-pane workspaces need not resemble collection pages. Keep frequent actions
reachable and let secondary regions yield as the window narrows.

Existing IA annotations use L1 for the header, L2 for page actions, L3 for the
toolbar, L4 for refinements and L5 for content. These describe locations, not a
mandatory stack. An explicit region name is better when those labels do not fit.

## 8. Voice and copy

Use [plain-language.md](plain-language.md) for voice, terminology and examples.
Judge the words together with their controls and surrounding information.

## 9. Accessibility

Preserve keyboard access, visible focus and meaningful control names. State
needs a cue beyond color; essential actions cannot depend on hover alone.
Review changed surfaces with longer translations, larger text and reduced
motion as relevant. Check contrast against the actual background, including art.

Existing white labels on bright orange keys, open port tabs and tags have an
accepted visual tradeoff (3.55:1 on Aluminium, 3.12:1 on Night). This is a known
contrast limitation, not proof that those surfaces meet a 4.5:1 target.

## 10. Controls and editing

Choose controls for the value and the work. A switch suits an immediate on/off
choice; a searchable select helps with data-driven options; segments suit a
few short, stable choices. Inline editing, a dialog or a dedicated workspace
can each be right depending on context and complexity.

Make the save model clear. Preserve useful input on failure, distinguish a
saved result from a pending edit, and explain consequential destructive actions.
A long value wraps rather than hiding a sentence behind an ellipsis; a path, or
a line that ends in one, stays on one line, gives way in the middle and shows in
full on hover (`kit::middle_line`, `kit::line_row`); a settings row's path can be
copied (`kit::path_row`).
See [shared patterns](design-patterns.md) for starting points.

## 11. Messages

Put feedback where it helps recovery: beside a field, in the current dialog,
in a content region, or in a notification when work finishes elsewhere.
Choose by the task rather than a universal toast rule. Avoid duplicate feedback
and retain enough context to explain what succeeded or failed. Keep technical
details available when useful without making raw diagnostics the headline.

## 12. The instrument: control language

Reuse the launcher's `key`, `controls`, `kit` and theme layers. Neutral surfaces,
keys, small LEDs, mono readings and restrained accent color give the product
continuity. Their composition can change with the feature; do not add a display,
caption, screw mark or card just to satisfy the metaphor.

Theme and component code own colors, geometry and states. Use semantic tokens
so custom themes and both appearances remain usable. Accent usually identifies
an important action or selection; danger identifies a destructive consequence.
Neither replaces a clear label or hierarchy.

## 13. Type

The existing palette uses Space Grotesk with system CJK fallback for interface
text, JetBrains Mono for values, DSEG7 for display digits and Monocraft for game
messages. Reuse the application's typography before introducing another face.
Choose size and weight for reading and comparison; page titles, compact tools
and technical views need different density. Text and controls must survive
translation and interface scaling without losing essential information.
