# Discover behavior

Contract for finding and installing content from Modrinth. Mapping ID
`CORE-DISC-001` names the read-only reference; endpoints and field shapes follow
Modrinth's public API documentation.

## Requests

- Every HTTP request carries `User-Agent: LumilioCL/<version>`; Modrinth
  rejects anonymous clients.
- Search is `GET /v2/search` with `query`, `facets`, `offset`, `limit`, `index`.
  `facets` is a JSON array of single-clause groups (groups are ANDed):
  project type, then game version (`versions:<v>`), then — mods only — the
  loader as `categories:<loader>`, then an optional category.
- `offset = page * page_size` (pages are zero-based); the page size is clamped
  to 1–100. Sort indexes: relevance, downloads, follows, newest, updated.
- Loaders are one facet group (any may match: `categories:fabric` OR
  `categories:quilt`) and only apply to mods and modpacks. Each chosen
  category is its own group (all must match). Changing anything but the page
  returns to page 1; a different project type forgets its categories and
  loaders (they differ per type) but keeps the game version.
- A project is `GET /v2/project/<id|slug>`; its versions are
  `GET /v2/project/<id|slug>/version?include_changelog=false`.
- The public page is `https://modrinth.com/<kind>/<slug>` (kind is `mod`,
  `modpack`, `resourcepack` or `shader`); it is only *linked* ("在 Modrinth 中打开"),
  never embedded. The detail page is native (below).
- Filter choices: `GET /v2/tag/category` (per project type, grouped by `header`)
  and `GET /v2/tag/game_version` (`version_type == release` marks full
  releases). Fetched once per run; a failure is not remembered. Project team:
  `GET /v2/project/<id|slug>/members`, owner = the `Owner` role (a project
  owned by an organization has none; the page simply omits the author).
- Read requests (search, project, versions, tags, members) go through the
  configured mirror chain like downloads do; POSTs (update checks) do not.

## Decoding

- Unknown members are ignored. Search hits of an unsupported project type, or
  without an id or title, are dropped. A project of an unsupported type is an
  error. Versions with no usable file are dropped (nothing to install).
- An unknown dependency type becomes `Other` instead of failing the list.
- Hits show Modrinth's `display_categories` when present.

## Choosing a version

Candidates must list the instance's game version. For mods they must also list
the instance's loader; resource packs and shaders declare loaders such as
`minecraft` or `iris` that say nothing about the instance, so they are not
filtered. Among candidates, release beats beta beats alpha, then the newest
publication wins. (The reference takes a version's first file; here the file
flagged `primary` is used, falling back to the first.)

## Install intents

- Mods go to `mods/`, resource packs to `resourcepacks/`, shaders to
  `shaderpacks/`, all under the instance's game directory. Modpacks are not a
  file drop; they become instances (later plan).
- The transfer id is `content:<project>:<version>`, verified by the published
  size and SHA-1.
- File names that are empty, `.`/`..`, contain a path separator, a colon, a NUL,
  or leading/trailing whitespace are refused before any network work.

## The list and the detail page

- A row shows the project's icon fetched by address (a pixel cover stands in
  while it loads, when there is none, or when the picture fails), title,
  author, two lines of summary, environment (client / server / both, from the
  two side flags; unknown says nothing), up to three categories (`+N` for the
  rest), loaders, downloads, follows and last update. Clicking the row opens
  the detail window; 安装 installs without opening it.
- The pager shows the first, last and current page with one neighbour each side;
  a single hidden page is shown instead of elided.
- Clicking a row opens the detail *in place of the list* inside the main
  window (no second window). 返回 or Esc returns to the same list: page,
  filters, sort and search text are kept. Choosing another landmark closes it.
- The detail has 介绍 (the project body, Markdown, rendered natively
  including images and badges), 版本 (every version: name, channel, game
  versions, loaders, date, downloads, size, and 安装 for mods / packs / shaders),
  and 画廊 (featured image first, then the author's order; a click opens the
  picture in the browser). The 更多 menu holds *Open in Modrinth*, *Copy link*
  and the project's source / issues / wiki / Discord links when it has them.
- A version that does not run on the chosen instance is marked and cannot be
  installed (the service refuses it too); a modpack installs as a new game.
