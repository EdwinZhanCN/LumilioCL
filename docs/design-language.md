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
| Type | None inside the art | Space Grotesk for Latin and figures, the system font for Chinese, JetBrains Mono for values, DSEG7 only on displays, Monocraft only for a server's message of the day (§13) |

The world carries the Minecraft identity. The interface is an instrument
held up to it: calm, legible, made of keys, LEDs and silkscreen (§12,
ADR 0013). Pixel fonts are never used for UI text, because they destroy CJK
legibility. A server's message of the day is the game's text, not the
interface's, so it alone is set in a pixel face (§13). Interface elements never imitate game GUI textures, and never
copy another maker's trade dress: no borrowed wordmarks, product names or
logos.

## 2. Motion tokens

Defined in `crates/lumilio-ui/src/theme/mod.rs` (`theme::motion`). Use the token,
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
three zones on one line. Each zone is its own floating surface on the content
column; the landmark capsule stays centred in the window, with the same gap on
either side.

| Zone | Shows | Rules |
|---|---|---|
| Leading | ‹ › and the current location's name | Browser-style history of *locations*: a landmark page, an Instance, a project detail. Tabs, filters and scroll position are view state, not history. A direction with nowhere to go is disabled (muted, arrow cursor), never hidden, so the name does not jump. A verified stable launcher update adds an orange restart key beside this zone; it is disabled while a game runs. Shortcuts: ⌘[ / ⌘], and Esc goes back from a detail. |
| Centre | The landmark capsule: Home, Library, Discover, Activity; Accounts and Settings join when their pages exist | Choosing a landmark opens a new location (and clears what was ahead). Icon-only keys; the open landmark sits pressed with its LED lit (§12); tooltip with label and shortcut. Activity carries the running-task badge: a lit LED with a mono count. |
| Trailing | The current account's face; the current instance: its cover, name and a ⇅ mark | Each chip opens a popover list to switch (a quick action, §10): the account who plays, the instance that play and Discover installs target. Switching never retargets an operation already started. With no account the chip reads "添加账户"; with no instances it reads "还没有游戏" and opens Library. The cover sits in a display window (§12), the same one a faceplate uses. A window too narrow for the widest trailing zone (a face and a full-length name) beside the centred capsule gets the compact zone: pictures only. The instance chip keeps its cover and ⇅; an empty chip is an empty face or cover with a +. Their words move into the tooltips. The window width decides, not the name, so switching instances never changes the bar. |

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
   - a list block (a panel with hairline rows drawn as §12 List rows; row
     actions are ghost or white keys);
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
| Discover | one line | — | — | 在 *source* 中浏览 · 重新搜索 | 整合包 / Mod / 资源包 / 光影 · 搜索 *source* | sort, page size, pager, filter sidebar (source segments at its top when several are on); a window whose content column cannot hold the sidebar beside a result row at its narrowest folds the sidebar into a **筛选** key before sort, with the number of picked filters in orange mono, that opens the same filters in a popover |
| Activity | running count | — | — | — | — | — |
| Instance | version · loader · last played | — | 启动游戏 | 安装游戏文件 · 复制… · 删除… | 概览 / 内容 / 世界 / 历史 / 日志 / 设置 | per-tab segments |
| Project detail | author · summary | — | 安装到 *instance* / 安装为新游戏 | 在 *source* 中打开 · 复制链接 · project links | 介绍 / 版本 / 图库 | version filters |

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
- **Accepted exception** (Project Maintainer, 2026-10-03): white labels on the
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
**技术详情** in a dialog that can copy it. Views of the main window post
toasts where results arrive and the shell shows them, whatever page is on
screen, so a result that arrives after its page was left is still told
(`lumilio-ui::toast`). A window of its own (the schematic studio) shows its
own.

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
never has two orange keys. Success uses a green LED and the green of
signal bars.

A server's message of the day is the one place the game's own colours
appear (List rows, below). They are the server's words, not accents of the
interface.

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
- A ghost key has no face at rest, so its side padding would indent its
  label from the column it starts. A ghost key that begins a line is flush
  (`Key::flush_start`): its label starts on the column's edge and its hover
  face reaches into the margin. Better still, a group's own action sits on
  the trailing side of its label (`kit::section_with`).

### State: LEDs, segment keys, faders

- An **LED** is a 6 px dot: orange with a soft glow when lit, LED-off grey
  when unlit. It always comes with a second sign of state (§9).
- **Segment keys** (refinements): a row of small white keys, each with an LED
  above it. The chosen key's LED is lit and the key sits pressed. They are
  for two to four fixed options with short labels the launcher writes. The
  keys grow with their labels and the LED band adds height, so options that
  come from data (region, scheme or file names) and a choice repeated on
  every row of a table are a Select instead, however few.
- **A row that mixes segment keys with plain keys** (the studio's
  Orbital / Explore beside 角度) aligns them on the keys' bottom edge: every
  key stands on one line, and the LEDs above mark the keys that hold a
  state.
- **A value chosen in a toolbar** from a few options (the discover page's
  排序方式, the studio's 切片) is a muted label and a small dropdown
  (`kit::labeled_select`): the dropdown shows the value in use, so it
  needs no LED.
- **Fader switch**: a black 4 px slot, a white 16 px round cap, an LED beside
  it. On means the cap is at the trailing end and the LED is lit.
- **Checkbox**: an 18 px key, white when clear, orange with a white check
  when set.
- **Radio**: the control is an LED in a 14 px recessed socket, lit when
  chosen.
- **Signal bars**: five rising 3 px bars for a server's round trip, lit
  green as in the game by its own steps (five under 150 ms, then one fewer
  at 300, 600 and 1000 ms); the tooltip gives the milliseconds. They appear
  only beside a server's player count.

### Port-label tabs

View tabs are flush blocks along the toolbar's leading edge: 30 px tall,
square corners, 1 px gaps, 12 px labels. Resting blocks are key grey; the
open block is orange with a white medium label.

### Fields

A field (search, dialog inputs) is a recessed panel: 4 px radius, a hairline
border, a faint inner shade along the top. A small field is as tall as a
small key (28 px at the base font size), so a field and the key that acts on
it line up in one row.

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

### Schematic studio

A plugin that names a schematics folder (`View::Studio`) gets the host's
studio (plan schematic-studio, D6). The studio is one view with two
placements (D22): a **catalogue** in the game page's plugin pane, and a
**workbench** in a window of its own, as the world map has. The catalogue
is for seeing what is there; the workbench is for working on one file
(D27). The page already stacks the page tabs, the plugin list and its own
pane, so a 3D viewport only fits in the window.

Three rules hold in both placements. They replaced a first studio that
was all tables and IDs (D27):
- **Pictures before words.** A schematic is shown by its thumbnail and a
  block by its icon, both drawn from the block assets (D29). Text says
  what a picture cannot: a name, a count.
- **Names, not IDs.** Blocks, items and entities carry the game's own
  names in the launcher's language (D28). An ID is a mono caption, a
  tooltip and a copy action, never the label. Block-state properties use
  the launcher's words (下半, 朝东), and a boolean that is false is not
  shown, except in the inspect card, which lists every property.
- **One fact, one place.** The file's name sits in the head only, and its
  size appears once. Facts for troubleshooting (the asset sources, volume,
  block entities, blocks that cannot be drawn) are folded under
  **技术信息**.

**The catalogue** fills the plugin pane's height. Its head holds 投影, a
mono caption with the file count, a search field and a white
**打开工作室** key with the maximize icon. Below it is a grid of cards in
sections by folder; the section label is the folder's name, and the root
has none. A card is:
- the thumbnail;
- the file name, two lines at most;
- a mono line with the size and the block count;
- a `≡n` mark when another file has the same blocks (a symbol and a
  number, never colour alone; the tooltip says how many files match);
- a plain version tag when the schematic's game version differs from the
  game's.

A click chooses a card (an ink ring). A double click or 打开工作室 opens
it in the window, or brings the open window forward on it. Card width
follows the pane: at least two columns at the 720 px window, never a
horizontal scroll. A thumbnail not yet drawn holds a panel-coloured place
and fades in (QUICK) when it is ready. The catalogue has no More and no
primary key: nothing is operated on here.

**The workbench** is one window per game and folder, 1120 × 760 by
default and 720 × 480 at least, under its own title bar (投影工作室). It
has a head and two columns.
- **Head**: the file switcher (a key with the 28 px thumbnail, the file
  name and a chevron, which opens a searchable list of the folder's files
  with thumbnails), then the size and the versions in one mono caption.
  On the trailing
  edge: the one orange key **替换方块…**, then More with 转换版本…,
  导出 PNG…, 导出材料清单, 编辑信息…, the reveal item the system names,
  and 删除… last in the danger colour (§7). An entry that cannot run now
  is disabled, and its tooltip says why (the game is running, a write is
  not done yet, or the file is still being read).
- **Viewport** (leading, the rest of the width): the model, with no
  border (a focus ring only while it has keyboard focus), no key row above
  it and no text under it. Two small tool plates float on its bottom
  corners. The leading one holds the camera's keys: the segment keys
  Orbital / Explore and 角度, a key whose menu holds 复位视角 and then the
  presets. The trailing one holds what is drawn: 切片, a labelled dropdown
  (不切片 / X 轴 / Y 轴 / Z 轴), and while slicing the layer, a number
  field with − and +, over it. Every control stands on one line (§12). A
  single strip of every key wrapped over the read-out at the window's
  minimum; the two plates fit side by side there. Above the leading plate,
  a read-out plate names what the pointer is on: the block's icon, name,
  property words and position. With nothing under the pointer it gives the
  controls in one muted line. While a block is inspected, its inspect card
  takes the plate's place.
- **Materials** (trailing, a share of the width within bounds): a search
  field and the summary ("35 种 · 1,743 个") over port tabs 材料 / 信息 /
  历史.
  - **材料** is a grid of block tiles, the most numerous first: the icon
    over the count in mono. Every block icon in the studio sits in a slot
    a little larger than it (key grey by day, the graphite key white by
    night), as the game draws items on inventory slots, so black and
    see-through blocks stay visible on a dark panel. The tooltip gives the name and the stacks
    ("1 组 + 22"). Choosing a tile pins a detail block to the column's
    bottom: the large icon, the name, the ID in mono with a copy key, the
    count and stacks, one chip per block state (its property words) with
    its count, and the ghost key **替换…** (the replace dialog on this kind,
    any state).
  - **信息** holds the file's name, author, description and times with a
    ghost **编辑…**, its entities (icon, name and count), its duplicates
    with **只保留这个**, and 技术信息 folded at the end.
  - **历史** lists the operations. Each line can be restored after a
    confirmation.

Writes go through the launcher's backup and log. The window has no
floating navigation: closing it is the way back, and its toasts appear in
it. A game starting or ending reaches the open windows as it reaches the
page. Choosing another file releases the old viewport's worker and
frames, as closing a modal does.

**A tile chooses a kind; a click in the picture inspects one block**
(D30, D31; after the Shulkr viewer and Litematica's info overlay: the
answer appears where the question was asked). The two never change each
other.
- **A kind**: hovering a tile keeps those blocks in colour and fades the
  rest to a quarter, desaturated; choosing the tile keeps the highlight,
  and the detail's **只看这种** hides the rest. A click passes through the
  hidden blocks.
- **One block**: a click on a block in Orbital, or on the crosshair's
  block while flying in Explore, outlines that block alone: a band of ink
  along the edges of the faces it shows, with a rim of the background
  inside it, so it reads on any texture. Its **inspect card** floats right
  over the block and moves with it as the camera turns (under it when the
  room over it is small, at the lower left when the block is out of view).
  It is as small as it can be: the icon and name with copy and close keys;
  the ID and position in one mono line; every property as a tag in the
  launcher's words, with `key=value` in the tooltip; for a container, its
  slots as the game's screen lays them out (rows of nine, a hopper's five,
  a dropper's three by three), each with its item's picture and count and
  the name and stacks in the tooltip; the given name and the text of a
  sign or command block. The rest of the block entity's data is behind
  技术详情. A card higher than the room over the block scrolls. The ghost
  key **选中这种** is the one way from the card to the kind. A click on
  empty space, Esc (after Esc releases a captured cursor) or the card's
  close key lets the block go.

**A slice and a box narrow the picture to what is being built** (T25;
after Litematica's render layers and area selection).
- **切片** draws one layer and those below it, from the top layer down,
  along the axis chosen in its dropdown (X / Y / Z 轴; 不切片 ends it).
  While slicing, the trailing plate adds the layer over the dropdown: a
  number field (`NumberInput`) whose − and + step a layer, which takes a
  layer typed in (past the schematic's first or last it stops there), and
  which follows `[` and `]` while the viewport has focus. How many blocks
  the layer holds is 材料's to say. The read-out plate and an inspect card
  in the corner rise to clear the taller plate. Keys typed into the field
  stay the field's: the picture's keys work only while it has focus. The cut shows each layer's own top faces,
  and a click passes through the layers above it. 材料 then counts that
  layer, under a mono line naming it.
- **A box**: a drag with Ctrl (⌘ on macOS) from one block to another
  selects every cell between them. The box's cells are washed a little
  toward the focus colour and a band of it runs along the box's edges, so
  it reads apart from the inspected block's ink. 材料 counts the box under
  the line "选区 4×3×6 · 52 块" with a ghost **清除选区**; Esc also clears
  it. 替换… opened with a box starts limited to it.

The fade and the outline change at once, with the next frame: they follow
the pointer, so an ease would only lag behind it. Blocks the game version
cannot draw are drawn as whole blocks in the missing texture (magenta and
black) and have a missing-texture tile, so the shape is complete and the
block can be chosen; their detail says so and offers the replacement.

**Versions are head facts.** The head names the schematic's game
version beside the game's own when they differ ("1.20.1 投影 · 游戏
26.2"). When the game's version lacks blocks the file has, the head shows
the region's one orange tag ("26.2 没有的方块：3 种") and a white
**转换到 26.2…** key; those tiles carry an orange ✕ (its tooltip
"26.2 没有这种方块") and the switch **只看不兼容** above them shows only
them. Conversion starts on the game's version and lists its result in
three groups, 丢失 / 近似 / 保留 (blocks the target does not know,
written as they are); under 保留 a ghost **为这些方块选替代…** hands those
blocks to the replace dialog. It saves to a new file beside the original
by default ("另存为「名 (26.2).litematic」，原文件不动", numbered when the
name is taken) and chooses it; the switch **覆盖原文件** writes over the
original instead. Restoring a new file's history line takes it into the
backups.

**Replacement starts from a block.** It opens from a tile's detail (any
state of the kind), from the inspect card's ghost **替换…** (that block's
state, every property exact), from the head's orange key and from
conversion's kept blocks. The dialog is a table of rows, 从 → 到, 640 px
wide, with no header: the arrow and the target's placeholder say which side
is which. A row is the source block with, under it in its column, the
switch **不限属性** and one chip per property, a line each (its name, and
on the column's trailing edge a dropdown: a value or 任意, so the dropdowns
stand in one column); a searchable picker over the file's game version's blocks
with the targets used last first (both pickers show block names whole with
the ID as a mono caption that gives way first, and search either), with the
row's own count in mono under it; and ✕, which every row has once there are
two. Every choice in the table is a dropdown (§12). The target picker runs
to the table's trailing edge or to ✕, so every row's starts and ends where
the others do. The target keeps
each property its blockstate names with the same value (a property no
model reads, such as `waterlogged`, takes its default). Rows are added with
the ghost **加一行**, flush with the table, and match what each cell held
before, together. The scope is a dropdown, 全部区域 / each region / 当前切片 /
选区, shown when there is more than the whole file. The commit key says the
total ("替换 128 个方块"); a line under the table speaks only when it has
something the counts do not say (still counting, nothing matches, a row
without a target). The 方案 act on the whole table, so they sit on the
footer's leading side: **套用方案…**, a menu of the kept ones once there is
one; **存为方案…**, whose popover asks for the name; then the scheme's name
while the table is that scheme (launcher data, every instance); a write
made from a 方案 names it in the history.

**No new display.** Counts (blocks, block types, entities) are mono
values on tiles, captions and tables, not segment digits. A display is
for progress and the play record only. If one page breaks that rule,
displays stop meaning "something is happening".

At the window's 720 × 480 minimum both columns stay visible, the grid
keeps at least four columns, and the two tool plates still stand side
by side, apart, while slicing too.

The model is game art. Its instructions, focus border, reset key and
failure messages are Interface. Rendering uses physical resolution, capped
proportionally at 4096 pixels per axis to bound GPU readback memory. Its
clear colour comes from the current panel token. A pair of segment keys
selects Orbital or Explore, and each mode keeps its own camera. Orbital
supports drag to rotate, wheel to zoom, arrows and +/−. Explore is
first-person free flight: click the picture or press Enter to capture the
cursor, then mouse look, WASD, Space to rise and Shift to descend. Movement
uses elapsed time and normalizes diagonals. There is no gravity or
collision. R, or 复位视角 (the modal's ghost key, the first entry of the
studio's 角度), resets the current camera; double-click resets only
Orbital. Esc releases capture first. Losing focus, switching
mode, switching file or a renderer failure releases the cursor and held
keys. Only captured first-person input schedules a frame clock; idle
pictures stay still. Intentional camera input stays responsive under
reduced motion. Animated blocks stay still. Native capture supports macOS,
Windows and X11. Wayland shows a calm unsupported explanation and keeps
Orbital (ADR 0036). Loading and failure occupy the viewport. Missing game
assets and no GPU get a calm explanation; other errors offer 技术详情. When
the assets cannot draw some blocks, one muted sentence under the viewport
says how many, with their IDs behind 技术详情 (ADR 0034). Replaced and
released frames are removed from GPUI's image cache (ADR 0028). A plugin's
`View::Model` still opens the same viewer in a modal.

In the studio the viewer adds three things (plan schematic-studio, T10). A
white 角度 key opens 复位视角（R）and, after a separator, the presets 北、东、
南、西、顶、等轴; each preset refits the orbit camera, and Explore gives way
to Orbital. In Orbital a click that
does not drag picks the first block along the ray and chooses its tile;
entities are not picked. 导出 PNG… in More asks
for a size (at most 4096 per side), a background (panel colour or
transparent) and an angle. The viewport's worker draws the picture
offscreen, and the host's save dialog decides where it goes. The picture's
resources (the bundle's game version and the overlays switched on) are
named once, in the 资源来源 list under 技术信息, not under the picture: a
long overlay list wrapped under the viewport and the same facts were
already in the list (D22).

The block art comes from the game itself, never from drawings of our own
(plan schematic-studio, D2/D3/D23). Vanilla blocks and entity geometry always
come from a pre-baked bundle of Mojang's assets that the launcher downloads
once from its own site, checks and keeps in its data folder; an installed
game is never baked, whatever its version. The bundle carries its own NOTICE.
Its download is a background task in Activity with one sentence, and
自动下载方块资源 in Settings turns it off. The instance's mods and resource
packs, and packs added in the studio, are each a row in 资源来源 with a fader
switch. All are off until switched on, so a schematic first looks as vanilla
draws it; a pack added from the studio is on in that folder. The switches are
kept per instance and studio folder, and each flip loads the viewport again.
The list shows first what the picture is drawn with: the base and the
sources switched on, in the order they are layered, with ↑ ↓ to change it
once two or more are on. The sources that are off follow under 未启用, with
only their switch: their place in the order draws nothing. A row is the
name on one line (cut short, the whole name in its tooltip) over a mono
caption: how many blocks the source provides and how many of its models a
custom loader draws; those show as missing, so the row says so before the
viewport does (T21). 添加… on the section's label offers a zip, a pack
folder or a typed path; the path is a dialog, which stays open with the
reason when the path is not a pack (T36).

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

### List rows

A row of a list block (worlds, servers, Home's places) never joins its facts
into one line with "·". A world or a server says the same things wherever it
is listed: Home's 接着玩 asks its servers how they are, as the Worlds tab
does. Each fact takes the form of what it is (`kit::Item`):
- the name, with its **state** beside it as an LED and a word: lit orange
  for what is happening now (正在游玩), green for works (在线), the failure
  colour for cannot be used (无法读取), unlit for not known or not there
  (正在检查, 无法连接);
- under the name, **tags** (a mode is ink: 生存, 极限; a version or a plain
  fact is plain: 作弊, 1.21.1) and then a **note** in muted mono (an address,
  a folder that differs from the name);
- beside the name, an **aside** in the item's own words: a server's
  message of the day, at most two lines without the space around them, in
  the pixel face (§13) with the game's colours and styles; without the
  pixel face it keeps the interface face and the colours. Each colour is
  darkened or lightened only until it reads on the page at 4.5:1, so the
  server's red stays red; uncoloured text is muted. The name block keeps
  280 px so asides start at one place from row to row; when the row is too
  narrow for both, the aside goes under the name. It has no window: a dark
  block in a light list is heavier than the message;
- on the trailing edge, **readings** in mono at a common minimum width, so
  rows keep their keys in line: a time ("3 天前"), a count ("12/100", the
  count in ink and the limit muted), signal bars;
- the row's keys last.

A row's leading picture is a 48 px display window (3 px corners), centred
on the row: the item's own picture, or a cover seeded by its identity.
Everything in a row is centred on one line.

A Discover result is the same kind of row, whatever the window size, so
the list shows as many results as the window can hold:
- an orange mono index, then the project's icon in its display window;
- the name (20 px light) with the author after it in muted mono on one
  line, the author cut first; one line of summary; one line of tags, where
  a tag with no room drops whole and the `+N` count stays;
- two readings stacked on the trailing edge with silkscreen legends: a count
  (FAV when the list is sorted by followers, else DL) and a date (PUB when
  sorted by newest, else UPD);
- the install key last, in a slot as wide as its widest state, so the
  readings line up from row to row.

### Faceplates

Instance cards are faceplates:
- **Panel**: 6 px radius, a hairline border, a soft shadow, a screw mark in
  each corner.
- **Top**: the name (20 px light) over the loader label (orange mono).
- **Display window**: the cover sits in it (3 px radius, no dissolve). This
  is the one place the world is framed; the navigation's instance thumbnails
  (28 px chip, 32 px popover rows) and list rows' leading pictures are the
  same window.
- **Bottom**: the game version as a plain tag and, on the trailing edge,
  when it was last played as a mono reading; then the key row.
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
| Messages of the day | Monocraft | 400, 700, italic | a server's message of the day, nowhere else |

- The four faces are embedded in `lumilio-ui` with their OFL licences and
  registered from memory at start-up, so loading them reads no file.
- The pixel face is not the game's font and is not read from the game. Its
  em is nine glyph pixels; it is sized so each glyph pixel is a whole
  number of screen pixels, nearest to 1.5 px at the interface scale
  (13.5 px on a Retina screen), with lines of twelve glyph pixels and its
  ligatures off. Characters it lacks, such as Chinese, fall back to the
  system font.
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
