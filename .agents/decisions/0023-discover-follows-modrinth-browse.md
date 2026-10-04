# 0023 — Discover follows Modrinth App's browse page

- Status: accepted
- Date: 2026-10-03

## Context

Discover had version / loader / category filters, five sorts and four page sizes. Modrinth App's
`Browse.vue` (with `browse-tab/*` and `utils/search.ts`) is richer in ways people notice:
include *and* exclude, environment, license, advanced exclusions, "hide already installed",
filters that a game provides and locks, tab rules inside a game, and card details. The
maintainer asked for the same information architecture and filtering / sorting policy, with the
source as the authority over `3rd-party/docs/modrinth-app-ia-diff.md`.

## Decision

- The search request is `/v3/search?new_filters=` with the app's filter expression
  (`SearchQuery::expression`, adapted from `utils/search.ts`; GPL-3.0-only, ADR 0022). Hits are
  decoded from the v3 shape (organization as author, published and modified dates, environment,
  a modpack's own loaders).
- Filters: game versions (several, snapshots behind "show all versions"), loaders (usual ones
  first), categories grouped by Modrinth's headers, environment, open source, and advanced
  exclusions (disclosures, plus plugin / data pack for mods). Options can be asked for or left out.
- Inside a game (opened from its Content tab) Discover *browses for that game*: the game's version
  and loader are provided to the search and locked until released (and a sync button), no pack
  tab, no mod tab for a vanilla game. There is no install header: the navigation already names
  where you are and goes back, and the corner chip names the game. Plain browsing from the navigation provides
  nothing and installs into the corner chip's game as before.
- "Hide already installed" on the pack tab (remembered) and inside a game (per visit). To know
  which packs the library came from, instances record `source_project` (schema 3, one column).
- What is remembered between visits: advanced exclusions, whether they are open, the
  photosensitivity warning dismissal, the pack tab's hide-installed (`Preferences::discover`).
- Project detail stays a single column and gains only what helps choosing a file: a filtered
  versions table (channel, loader, game version; prefiltered to the game when coming from one) with
  install / switch / installed per row and the version ranges, the install button's states
  (install, installing, update, switch version, installed), a right-click menu, a gallery tab
  only when there are images, with a viewer. Everything that belongs to the Modrinth platform
  (links, creators, disclosures, license text, changelogs, dependency and file lists, reports)
  stays one click away in "在 Modrinth 中打开". A first attempt with a sidebar and a version page
  was removed after review: it crowded the column and mostly duplicated the website.
- Cards: tags ordered and folded like the app's, environment wording, publishing date when sorted
  by newest, right-click menu (open / copy link), an "installing" state.

## Not done, and why

- Servers tab: tied to Modrinth hosting and ping infrastructure.
- Data Packs tab: Modrinth installs them into `<game>/datapacks`, which vanilla does not read
  (it relies on its own global-datapack feature). Needs world-level data pack management first.
- Install queue and grid layout: the app uses the queue only for server context and passes no
  display-mode toggle, so list + immediate install is already the app's behavior.
- "Depends on / Included content" filter (needs a project picker).
- Installing a version that does not fit the game after unlocking a filter: the core refuses
  that on purpose, so unlocking only widens what is *shown*.

## Consequences

- Positive: filters behave as people coming from Modrinth App expect; the filter expression is a
  pure function with tests; locked filters prevent browsing for the wrong game by accident.
- Negative / trade-offs: the instance database moved to schema 3 (older builds refuse it; a `.v2`
  copy is kept on upgrade). Discover now depends on `/v3/search`, which Modrinth documents less
  than v2.
- Shipped: plan `discover-modrinth-parity`.
