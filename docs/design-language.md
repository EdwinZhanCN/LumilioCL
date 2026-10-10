# LumilioCL Design Language

This is the living UI/UX specification for LumilioCL: it decides *how* the
launcher *looks, moves, and speaks*. Every UI or
motion change follows it through the `lumilio-motion-design` skill
(`.agents/skills/lumilio-motion-design/SKILL.md`). Changing a rule here is a
design decision: update this file in the same change and say why.

## 1. The one rule: the world steps, the interface flows

LumilioCL has two visual registers and never mixes them.

| | World | Interface |
|---|---|---|
| What | Windows into the game: the Home hero, instance covers, empty-state vignettes, launch loading | Chrome: navigation, buttons, lists, dialogs, text, route changes |
| Rendering | Procedural pixel art (`lumilio-ui::hero` raster), no bitmaps | The instrument elements in `kit` (§12) over gpui-component, theme tokens |
| Motion | Discrete, at `WORLD_TICK` (20 ticks/s, the game's own rate); state changes stream in chunk by chunk | Continuous easing and springs from the motion tokens |
| Framing | Full-bleed; meets the page through a dithered dissolve into the live theme background; never boxed in a hard-cornered card. The exception is a display window (§12): a faceplate's cover, and the navigation's instance thumbnails | Instrument shapes: keys and fields 4 px, panels and displays 6 px |
| Appearance | Identical in light and dark; foreground on art is always light-on-dark over the scrim | The body follows the appearance (aluminium / night); keys are objects and keep their colour (§12) |
| Type | None inside the art | Space Grotesk for Latin and figures, the system font for Chinese, JetBrains Mono for values, DSEG7 only on displays (§13) |

The world carries the Minecraft identity. The interface is an instrument
held up to it: calm, legible, made of keys, LEDs and silkscreen (§12,
ADR 0013). Pixel fonts are never used for UI text, because they destroy CJK
legibility. Interface elements never imitate game GUI textures, and never
copy another maker's trade dress: no borrowed wordmarks, product names or
logos.

## 2. Motion tokens

Defined in `crates/lumilio-ui/src/theme.rs` (`theme::motion`). Use the token,
never a literal duration.

| Token | Value | Use |
|---|---|---|
| `INSTANT` | 90 ms | hover, press, focus feedback |
| `QUICK` | 160 ms | route content fade + 8 px lift, small state swaps |
| `SETTLE` | 280 ms | panels, button → progress morph, sheets |
| `SCENE` | 700 ms | hero copy entrance, large narrative changes |
| `WORLD_TICK` | 50 ms | frame interval of every world-register animation |
| `LIFT` | 8 px | entrance travel distance |

Easing: entrances use `ease_out_quint`; exits are shorter than entrances and
never bounce; navigation selection uses a spring. Nothing loops in the
interface register except an explicit busy indicator.

Instrument motion (§12):
- A key travels down 1 px and back within `INSTANT`.
- An LED switches on or off within `INSTANT`, and never blinks except as the
  busy indicator.
- A fader cap slides over `QUICK`.
- Display digits follow the displayed progress, and the bar meter lights
  whole bars.

## 3. Motion grammar

- **Chunk loading means "the world is becoming ready".** It is used for hero
  slide changes and for the launch moment, where the chunks revealed track
  real launch progress. Do not reuse it for unrelated waiting.
- **Progress never moves backwards.** A bar, a chunk reveal, or a count may
  stall; it may not retreat. The core `LaunchSession` enforces this.
- **Displayed progress eases toward reported progress** (exponential follow),
  so bursty events still read as continuous loading.
- **One thing moves at a time** in a region. Staggered entrances are fine;
  simultaneous unrelated motion is not.
- **Reduced motion** (`App::reduce_motion`) shows a composed still frame,
  disables auto-advance, and turns morphs into immediate swaps. Every
  animation must degrade this way.

## 4. Performance is part of the experience

- World animations tick at `WORLD_TICK`, only while visible: a ticker
  notifies only if the view rendered within a short grace window, so hidden or
  occluded views cost nothing.
- **While the game runs, the launcher is still.** No world animation, no
  ticking faster than once per 20 s (for the play-time readout).
- Large texel grids are painted as vertically merged rects inside one
  `Window::paint_layer`, never one quad per texel.
- Budget: an animating Home stays under ~10 % of one core in release.

## 5. Home

Home is the fastest route from "open the launcher" to "playing". The hero is
not decoration; it reflects Home's state.

Layout: the window uses a transparent title bar, so Home's world runs
full-bleed from the top edge under the traffic lights, takes `HERO_SHARE` of
the window height (never less than `HERO_MIN_HEIGHT`), and dissolves into the
page over `HERO_FADE`. Hero copy, Home foreground, and page content share one
content column (`CONTENT_MAX_WIDTH`, `CONTENT_PADDING_X`), so text over art and
text on the page line up. Page content below the world uses theme colours and
compact rows: first use is one line of copy with its two actions. For a game,
top to bottom:
- what needs attention;
- **接着玩**: the continued game's latest worlds and first servers, a list
  block whose rows go straight in (Quick Play), beside its **游戏记录**, a
  record display (§12);
- the recent games as the Library's own faceplates, one row.

Below the world is where Home stops being empty for a person with one game,
so every group there is something to enter, never decoration.

| Home state | Hero | Foreground |
|---|---|---|
| Ambient / first use | Showcase carousel of all scenes | First-use actions below the hero |
| Continue | Continue's own scene, 营火 Hearth: a night camp whose fire still burns, with the instance's world as a landmark on the far hill (village, portal, mine, redstone lamp). The fire flares while the pointer rests on **继续** | Instance name, metadata, one primary **继续** on the art; recent cards below |
| Launching | Same scene, loading chunk by chunk with real progress | The **继续** key morphs into a display (§12): stage name, segment-digit item count, bar meter; stage stepper, **取消** |
| Playing | A still, dimmed frame (no ticking) | "正在游戏中", play time, **结束游戏** |
| Recovery | A calm, still night scene | What failed, in plain words; **恢复并继续** and **技术详情** |

Launch stages shown to people, in order: 检查 → 依赖库 → 资源 → 启动.

### Controls on world art

Keys on art are the same objects as keys on the page (§12), so they look the
same in both appearances:
- The primary is the orange key, the one bright object on the art.
- Secondary actions are black keys.
- A key group on art may carry silkscreen captions and LEDs, like a control
  panel.

Keys are our own elements, not `ButtonCustomVariant` (postmortem 0001).
Nothing on the art stretches to the column width; every control keeps its
intrinsic size.

### Appearance

The launcher follows the system appearance, at start and live
(`follow_system_appearance`). World art does not change with appearance; the
page it dissolves into does.

## 6. Navigation

The navigation bar floats at the bottom of every page, Home included, as
three zones on one line (ADR 0012). Each zone is its own floating surface on
the content column; the landmark capsule stays centred in the window.

| Zone | Shows | Rules |
|---|---|---|
| Leading | ‹ › and the current location's name | Browser-style history of *locations*: a landmark page, an Instance, a project detail. Tabs, filters and scroll position are view state, not history. A direction with nowhere to go is disabled (muted, arrow cursor), never hidden, so the name does not jump. A verified stable launcher update adds an orange restart key beside this zone; it is disabled while a game runs. Shortcuts: ⌘[ / ⌘], and Esc goes back from a detail. |
| Centre | The landmark capsule: Home, Library, Discover, Activity; Accounts and Settings join when their pages exist | Choosing a landmark opens a new location (and clears what was ahead). Icon-only keys; the open landmark sits pressed with its LED lit (§12); tooltip with label and shortcut. Activity carries the running-task badge: a lit LED with a mono count. |
| Trailing | The current instance: its cover, name and a ⇅ mark | The instance that play and Discover installs target. Clicking opens a popover list to switch (a quick action, §10). Switching never retargets an operation already started. With no instances it reads "还没有游戏" and opens Library. The cover sits in a display window (§12), the same one a faceplate uses. |

Location names: a landmark's label; an Instance's own name; a project detail
by its kind — "整合包详情", "Mod 详情", "资源包详情", "光影详情". Long names
truncate; the full name is the tooltip.

Pages never draw their own back button: going back is the navigation's job.

Still planned: the active landmark expands to show its label with a spring;
Activity shows aggregate progress as a thin ring, completion gives one small
pop, failure turns the badge to `danger`.

## 7. Page anatomy

Every page except Home is built from the same layers, top to bottom. Omit a
layer that has nothing in it; never reorder or merge layers.

1. **Header** — the title (36 px light, §13) and one caption line (muted, one
   sentence; prefer a live fact such as "3 个游戏" or "2 件事正在进行"; the
   number in the fact is mono and orange).
   Detail pages (Instance, project) lead with their media — cover or icon —
   and may add pills and stats under the caption.
2. **Page actions** — on the header's trailing edge, bottom-aligned with the
   caption. At most three controls, always in this order:
   **secondary** (white key) · **primary** (orange key) · **more** (black
   key, icon-only ⋯, opens a menu). Page actions carry no silkscreen
   captions. One primary per page at most, and a page with no natural
   primary has none — never promote an action just to fill the slot. Every
   further action lives in More; destructive items come last in More, in the
   danger colour, and confirm in a dialog.
3. **Toolbar** — one row: the view tabs (port labels, §12; the page's
   first level) on the leading side and search on the trailing edge (240 px; the placeholder
   names what is searched). A page without tabs still puts search on the
   trailing edge. Search never gets a row of its own.
4. **Refinements** (optional) — controls that narrow or order the content
   without changing what it is: segment keys with LEDs (§12; the level
   below the tabs), sort, page size, pager, a filter sidebar.
5. **Content** — one of:
   - a card grid (Library; instance cards are faceplates, §12);
   - a list block (a panel with hairline rows; row actions are ghost or
     white keys);
   - settings groups (§10).

   Loading, empty and error states replace the content in place, never the
   header or the toolbar.

Messages about what just happened never sit in the page (§11).
Context that is not an action stays out of the header — the install target,
for example, is the navigation's current instance (§6), not a line on
Discover.

| Page | Caption | Secondary | Primary | More | Tabs · search | Refinements |
|---|---|---|---|---|---|---|
| Library | game count | 导入整合包 | 新建 | — | 全部游戏 / 收藏 · 搜索游戏 | — |
| Discover | one line | — | — | 在 Modrinth 中浏览 · 重新搜索 | 整合包 / Mod / 资源包 / 光影 · 搜索 Modrinth | sort, page size, pager, filter sidebar |
| Activity | running count | — | — | — | — | — |
| Instance | version · loader · last played | — | 启动游戏 | 安装游戏文件 · 复制… · 删除… | 概览 / 内容 / 世界 / 历史 / 日志 / 设置 | per-tab segments |
| Project detail | author · summary | — | 安装到 *instance* / 安装为新游戏 | 在 Modrinth 中打开 · 复制链接 · project links | 介绍 / 版本 / 图库 | version filters |

### Covers and empty states

- Instances get deterministic pixel covers from the hero raster, seeded by the
  instance id and themed by loader/pack type (`lumilio-ui::cover`). Covers are
  static today; only the hovered card may animate (not yet built). A card's
  cover is its faceplate's display: it sits in a display window and does not
  dissolve (§12). The Instance page's cover is the
  header's backdrop: it sits in the top-right corner under the title bar and
  page actions, and dissolves leftward and downward into the page, so the
  title always sits on the page colour and the cover costs no height.
- Starting an install from Discover sends the item's icon to the Activity
  landmark (the game's item-pickup motion) and the task appears there.
- Empty states are a single small vignette with one moving element and one
  sentence, never an illustration wall.

## 8. Voice and copy

- Simplified Chinese first, English beside it; both calm and direct, second
  person implied: "把原来的游戏带过来", "上次没有启动成功".
- Errors: one plain sentence saying what happened, one primary action, and
  technical detail behind **技术详情** (*Technical details*). Never blame the
  user; never show raw error strings as the headline.
- English is written, not transcribed: say what an English launcher would
  say. The Chinese is the source for facts, not for phrasing.
  - Sentence case everywhere: titles, tabs, keys, menu items ("Download
    recommended Java"). Proper nouns keep their capitals (Java, Fabric,
    Modrinth).
  - An action that asks for more before it acts ends in one "…" character
    ("Export…", "Add Java…"), as the Chinese does. No exclamation marks.
  - English runs 1.5–2× the width of the Chinese. Keys, segments and tabs
    never take a width sized to the Chinese; every page is checked in
    English at the 720 px window.
- Things the system names, the launcher names the same way on each system:
  "在访达中显示 / Show in Finder" on macOS, "在文件资源管理器中显示 / Show in
  File Explorer" on Windows, "在文件管理器中显示 / Show in file manager" on
  Linux (`platform::reveal_label`).
- Numbers only when they help decide something ("128 / 342"), not as noise.
- Game mechanics may be referenced playfully in world-register copy (hero
  captions), never in actionable UI.
- **Silkscreen may be Latin** (§12), within limits:
  - Allowed: uppercase legends that echo an adjacent Chinese label (PLAY
    under **启动游戏**), proper nouns (FABRIC, NEOFORGE), versions, numbers
    and units (MB, MODS, STAGE 3/4).
  - Not allowed: a Latin word as the only name of an action or the only
    statement of a fact. Labels, headings, values and messages are in the
    interface language.
  - In English the legends stay, even where one repeats its label (PLAY
    under **Play**): silkscreen is part of the instrument, not a translation.

## 9. Accessibility

- Every icon-only control has a tooltip label; shortcuts appear in tooltips.
  Labelled buttons have no tooltip — it would only repeat the label.
- Every enabled, clickable control shows the pointing-hand cursor; disabled
  controls keep the arrow. Use `theme::clickable` for gpui-component buttons,
  which default to the arrow; plain clickable `div`s use `.cursor_pointer()`.
  Non-interactive surfaces (e.g. recent cards without an action) keep the
  arrow.
- Focus is always visible (`theme.ring`); carousels are operable without a
  pointer.
- Text over world art sits on the dithered scrim and keeps ≥ 4.5:1 contrast.
- **An LED is never the only sign of state.** The chosen segment key also
  sits pressed with a semibold label, a fader's cap moves, a chosen radio's
  label stays ink, and the open port tab is filled.
- Text reaches 4.5:1 against what it sits on, accent text included. Orange
  and danger as text use their text tokens (§12).
- **Accepted exception** (maintainer, 2026-10-03): white labels on the
  bright orange fill measure 3.55:1 on aluminium and 3.12:1 on night. They
  are kept for the look, and only on orange keys, the open port tab and the
  orange tag. A darker fill (`#C94108`, 4.95:1) was tried and rejected as too
  dull. If the exception has to go, switch
  those labels to ink (5.3:1 / 6.0:1), not the orange to a darker shade.
- Non-text marks reach 3:1 where they are the only sign of something. A lit
  LED on aluminium is 2.8:1, which is allowed only because an LED is never
  the only sign of state.
- Reduced motion is honoured everywhere (§3).

## 10. Controls and editing

The page surface shows values and offers **quick actions**; it is not a form.

- **Quick actions stay on the page**: fader switch, Select / dropdown, LED
  radio or segment keys, a single-shot key, the More menu (§12). Choosing from a
  rich list (the current instance) is a popover anchored to its trigger.
- **Free-form edits open a dialog**: text, numbers, paths, argument lists,
  and multi-input flows such as copying an instance. The page row shows the
  current value read-only with a ghost **编辑** on its trailing side. One
  dialog per logical group ("内存" edits minimum and maximum together).
- **Dialog shape**: title "编辑…" (or the flow's verb), fields stacked
  vertically with the label above each input, footer **取消** (outline) and
  the commit (primary). While the write runs, the dialog stays open and
  disabled; a failure stays in the dialog as one sentence under the fields;
  success closes it and the page shows the new value. Esc and the overlay
  cancel; they never save.
- **Settings groups**: a silkscreen section label over one hairline table
  (§12). Each row has the
  label (plus info button) on the leading side and the value or quick control
  on the trailing side. Rows stack vertically — never two settings side by
  side in one row, never label-above and label-beside mixed in one group.
- **Help hides behind an info button**: no explanatory paragraph under a
  label. A muted ⓘ right after the label shows the help as a tooltip (one or
  two sentences); longer help uses a popover. Inherited values say so in the
  value itself: "跟随默认 · 4096 MB", "由 Java 决定".
- **Destructive actions confirm in an alert dialog** that states the
  consequence in one sentence, with the confirm key in the danger colour.
  The danger colour must never be mistaken for the signal orange (§12).
  Existing inline two-click confirmations migrate to this (ADR 0012).

## 11. Messages

Every piece of feedback has one home, and none of them is the page column.

| What | Home | Behaviour |
|---|---|---|
| Work in progress | The control that started it (busy, disabled) and Activity | Never a line of text on the page. A long background task (install, import) may announce its start once as an info toast: "开始安装 X，进度在动态里". |
| Result of an operation — done, cancelled, failed, copied | A toast (gpui-component notification, top right under the title bar) | Info and success hide themselves. A failure stays until dismissed: one plain sentence, and **技术详情** when there is a raw cause. Startup recovery notes stay too. |
| A failure inside an open dialog | The dialog, under its fields | No toast as well; the draft stays (§10). |
| State of the page — loading, empty, could not load | The content area, in place of the content (§7) | One sentence, **重试** where it helps, **技术详情** beside it. |

Raw error text is never a headline and never page copy: it opens from
**技术详情** in a dialog that can copy it. Views queue toasts where results
arrive and show them on their next render (`lumilio-ui::toast`).

The crash-analysis dialog is itself a technical reading surface: evidence and
analysis failures appear inline under **技术详情**, alongside the findings,
without another dialog. Its footer copies the complete redacted analysis snapshot.


## 12. The instrument: control language

Adopted by ADR 0013 and implemented by plan 0031.

The interface is drawn as a hand-held instrument: a neutral body, physical
keys, LEDs for state, silkscreen for grouping, a black display for live
numbers. Every element below is a `kit` primitive; pages never draw them by
hand.

### Bodies and the signal colour

| Token | Aluminium (light) | Night (dark) | Use |
|---|---|---|---|
| page | `#E4E4E1` | `#111111` | the body |
| panel | `#EEEEEB` | `#1B1B1B` | faceplates, list blocks, fields |
| hairline | `#BCBCB7` | `#353533` | rules, borders, brackets |
| ink | `#121212` | `#ECEBE7` | text, table top rules |
| muted | `#666662` | `#8C8B86` | captions, silkscreen, secondary values |
| key white | `#F9F9F7` | `#313130` (graphite, lit top edge) | secondary keys, segment keys, fader caps |
| key black | `#1D1D1D` | `#2A2A2A` | More, secondary keys on art |
| key grey | `#D3D3CF` | `#262626` | resting port tabs, disabled keys, plain tags |
| signal orange (fill) | `#F0520F` | `#FF5A1A` | orange keys, the open port tab, orange tags, lit LEDs; white labels on it are an accepted exception (§9) |
| signal orange (text) | `#B53C08` | `#FF5A1A` | orange used as text: 4.6:1 on aluminium, 6.1:1 on night |
| danger (fill) | `#C8261B` | `#C8261B` | the destructive confirm key; white text on it is 5.6:1 |
| danger (text) | `#B42318` | `#FF5A4F` | destructive menu items, failure text: 5.2:1 on aluminium, 6.1:1 on night |
| LED off | `#A3A39E` | `#3A3A38` | unlit LEDs |
| display | `#141414` | `#070707` | display windows |
| display ink / dim | `#FF6A2A` / `#3D2619` | `#FF6A2A` / `#3A2418` | lit and unlit segments and bars |
| display label | `#948479` | `#948479` | legends and units on a display (4.5:1 or better) |

**Signal orange** is the only accent. It marks:
- the primary key;
- the open port tab;
- a lit LED;
- numbers that matter: the count in a caption, a section index, the live
  value in a table;
- a faceplate's loader label;
- one emphasis tag per region.

Orange that fills something uses the fill token; orange that is read as
text uses the text token. On night they are the same colour; on aluminium
the bright fill reads at only 2.8:1, so text takes a deeper orange. It never fills decoration, never sets body text, and a region
never has two orange keys. Success uses a green LED.

**Danger** sits only 14° of hue from the signal orange, so the two are kept
apart by place, not by hue alone:
- danger appears only in a destructive confirm (the alert dialog's commit
  key, beside a white **取消**), on the trailing destructive items of a More
  menu, and on a failure LED;
- none of those places ever holds an orange element;
- a danger key always names the destruction ("删除"), never "确定".

### Keys

- Kinds: **white** (secondary), **black** (More; secondary on art),
  **orange** (primary), **ghost** (a text key for row actions such as
  **编辑**, with a key-grey hover), **disabled** (key grey, muted label, no
  shadow, arrow cursor).
- Shape: 34 px tall, 4 px radius, 16 px side padding (square when icon-only),
  14 px medium label, 14 px icon.
- At rest a key has three shadows: a hard 1 px contact shadow, a soft 3 px
  cast shadow, and a lit top edge. Pressed, it travels down 1 px and its face
  falls into shade. Hover shifts the face a step.
- Keys are objects. At night the white key is graphite with a lit top edge,
  as on the reference hardware: white is reserved for lit things, so a night
  key never glares against the body.

### State: LEDs, segment keys, faders

- An **LED** is a 6 px dot: orange with a soft glow when lit, LED-off grey
  when unlit. It always comes with a second sign of state (§9).
- **Segment keys** (refinements): a row of small white keys, each with an LED
  above it. The chosen key's LED is lit and the key sits pressed.
- **Fader switch**: a black 4 px slot, a white 16 px round cap, an LED beside
  it. On means the cap is at the trailing end and the LED is lit.
- **Checkbox**: an 18 px key, white when clear, orange with a white check
  when set.
- **Radio**: the control is an LED in a 14 px recessed socket, lit when
  chosen.

### Port-label tabs

View tabs are flush blocks along the toolbar's leading edge: 30 px tall,
square corners, 1 px gaps, 12 px labels. Resting blocks are key grey; the
open block is orange with a white medium label.

### Fields

A field (search, dialog inputs) is a recessed panel: 4 px radius, a hairline
border, a faint inner shade along the top.

### Silkscreen

- **Section label**: an orange mono index ("01", on pages with several
  groups such as Settings; `kit::section_at`) and the ink title, with no
  rule after it. Settings groups and other titled groups use it.
- **Captions** are 10 px muted legends under keys (§8 limits their wording).
  They belong only to key groups that act as a control panel, such as Home's
  foreground and the launch controls. A stateful key's caption carries its
  LED.

### Displays

Live numbers sit on a black display: 6 px radius, an inner shadow. A display
shows:
- a mono stage legend ("STAGE 3/4");
- the stage name in display orange;
- the count in segment digits (DSEG7) over unlit "8"s, with its unit in muted
  mono;
- a bar meter of 4 px bars that light one whole bar at a time.

Displays follow the progress rules in §3. Home's launch moment and Activity
use them for progress. The one other display is Home's **游戏记录**: a record,
not progress, so it has readings (a label in the legend colour over segment
digits and a mono unit) and no meter, and nothing on it moves. Nothing else is
a display.

### Native model preview

The projection detail places the “打开 3D 预览” key and its other actions inline
with the title. The material table shares the detail pane's scroll rather than
owning a nested scroll region. A floating “返回顶部” key stays at the pane's
lower right and returns that shared scroll to the title immediately.
It opens the same modal family as the screenshot viewer and loads assets only
for that modal session. Width and viewport height follow the available window,
including the 720×480 minimum. Rendering uses physical resolution, capped
proportionally at 4096 pixels per axis to bound GPU readback memory. Its clear
colour comes from the current panel token.

The model is game art; its surrounding instructions, focus border, reset key
and failure messages are Interface. A pair of segment keys selects Orbital or
Explore and preserves each mode's camera independently. Orbital supports drag
to rotate, wheel to zoom, arrows and +/−. Explore is first-person free flight:
click the picture or press Enter to capture the cursor, then mouse look, WASD,
Space to rise and Shift to descend. Movement uses elapsed time and normalizes
diagonals; there is no gravity or collision. R or the ghost reset key resets the
current camera; double-click resets only Orbital. Esc releases capture first,
then closes the modal and returns focus to its entry key. Closing, switching
mode, losing focus or a renderer failure releases the cursor and held keys.
Only captured first-person input schedules a frame clock; idle pictures remain
still. Intentional camera input stays responsive under reduced motion. Animated
blocks remain still. Native capture supports macOS, Windows and X11; Wayland
shows a calm unsupported explanation and retains Orbital (ADR 0036).
Loading and failure occupy the same viewport. Missing game assets and no GPU
receive a calm explanation; other errors offer 技术详情. When the game's assets
cannot draw some blocks, one muted sentence under the viewport says how many,
with their IDs behind 技术详情 (ADR 0034). Replaced and released frames are
explicitly removed from GPUI's image cache (ADR 0028).

Account details use a CPU-rendered player preview alongside a wardrobe,
inside the Accounts master–detail page rather than a
navigation landmark. The look has two layers, after Modrinth App's skin page.
The first is the figure, unframed, with one place to act under it, beside a
grid of skin pictures. The grid's first cell adds a skin (choose or drop a
PNG); each other cell is only a picture, a name and ⋯ (rename, edit, move,
remove). The worn skin carries a tick; a tried-on skin is ringed and the
figure is badged 试穿中. Choosing a picture only changes the local preview,
and the place under the figure becomes Revert and Apply. Otherwise it holds
Edit appearance, with ⋯ beside it for a Microsoft account (save the current
skin to the library, restore the default skin, refresh). The account head
shows the name and kind; the UUID is copied from its menu.
The second layer replaces the grid while the figure follows the draft:
texture, arm model and cape pictures are edited together, Apply to account
and Save to library name their target, an unchanged draft cannot be
submitted, and Cancel or Escape restores the previous preview and returns
focus to Edit appearance. An offline account's skin source (default, local
file, LittleSkin, custom site) is that same layer; it still uses the local
skin server at launch. When a read fails or the shown profile is stale,
Refresh appearance also sits beside the explanation under the figure. A
failed read keeps the last picture and says it was not updated. Pictures are
keys: they take focus, and Enter or Space acts as a click. Third-party
accounts preview session textures and link to the skin site. Local skin
imports copy normalized PNGs into the launcher library; removing a library
entry leaves any already selected offline file usable. The wardrobe and
preview belong to one retained detail session, so late reads cannot replace a
newly opened account. Profile reads and texture downloads are cached per
account and per texture address. Accepted Microsoft writes show pending
feedback while one delayed profile read confirms propagation; that delay does
not prove the change has reached every Minecraft server. Default artwork
comes from an installed client jar; missing artwork leaves a grey model with
an explanation. The account dropdown opens the current account's detail.

The preview shares the Orbital input and frame-cache lifecycle.
It draws only when the look, camera, equipment shape
or viewport changes. Dragging right turns the face to the screen's right,
as Orbital does; R or a double-click resets the camera, with no reset key.
With a cape texture, a small Elytra key in the viewport's corner switches
the preview shape without changing the account's worn equipment.
Changing looks clears the previous image while the new frame is drawn.

### Tables

Key/value lists (settings groups, launch stages, version lists) are hairline
tables with no surface:
- an ink rule on top and a hairline under each row;
- the label leading, the value trailing in mono;
- the live or current value in orange, with the ink rule moved under its row.

### Tags

Tags are 20 px tall, 2 px radius, 11 px mono:
- **ink** (a proper noun such as the loader);
- **plain** (key grey, versions);
- **orange** (at most one per region).

A status is not a tag: it is an LED and a word ("● 可以启动").

### Faceplates

Instance cards are faceplates:
- **Panel**: 6 px radius, a hairline border, a soft shadow, a screw mark in
  each corner.
- **Top**: the name (20 px light) over the loader label (orange mono).
- **Display window**: the cover sits in it (3 px radius, no dissolve). This
  is the one place the world is framed; the navigation's instance thumbnails
  (28 px chip, 32 px popover rows) are the same window.
- **Bottom**: a silkscreen meta line and the favourite LED.
- Hover turns the border ink.

Screw marks appear on faceplates only.

### Restraint

The instrument is quoted, not imitated. Nothing below appears in the
interface:
- knobs that do not turn;
- speaker grilles;
- fake labels on ports.

The skeuomorphic details listed above are the whole set.

## 13. Type

| Role | Face | Weights | Where |
|---|---|---|---|
| Latin and figures | Space Grotesk | 300, 400, 500, 700 | the interface's default family |
| Chinese | the system font, through fallback (PingFang SC on macOS, Microsoft YaHei UI on Windows, the distribution's CJK sans on Linux) | as the platform provides | every Chinese glyph; not bundled (ADR 0013) |
| Values | JetBrains Mono | 400, 500 | versions, sizes, units, counts, tags, section indices, table values |
| Displays | DSEG7 Classic | 400 | segment digits on a display, nowhere else |

- The three faces are embedded in `lumilio-ui` with their OFL licences and
  registered from memory at start-up, so loading them reads no file.
- Sizes:

  | Text | Size and weight |
  |---|---|
  | Page title | 36 px light |
  | Faceplate name | 20 px light |
  | Body and values | 14 px regular |
  | Key labels | 14 px medium |
  | Port tabs and tags | 11–12 px |
  | Silkscreen | 10 px |
  | Display digits | 28 px |

- Chinese takes whatever weight the platform's system font has. Where it has
  no light weight, titles render in regular rather than in a bundled face.
