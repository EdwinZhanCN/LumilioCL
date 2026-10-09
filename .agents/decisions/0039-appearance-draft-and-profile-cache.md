# 0039 — Appearance edits are a draft; profiles and textures are cached per identity

- Status: accepted
- Date: 2026-10-08

## Context

Account details show a CPU player preview and a wardrobe (ADR 0024, ADR 0028).
Microsoft appearance is read and written through the Mojang profile API. Opening
the detail, drawing the preview and listing capes would otherwise each fetch the
same profile and the same texture URLs. Modrinth App caches a profile for 60
seconds, backs off on 401 for the same token, and debounces skin writes by 10
seconds (`packages/app-lib/src/state/minecraft_auth.rs`,
`packages/app-lib/src/api/minecraft_skins.rs`). That debounce is not a guarantee
that Minecraft's session servers have observed the write. Lumilio already waited
11 seconds before one confirmation read; that wait is this launcher's own
initial policy.

## Decision

- The look has two layers, after Modrinth App's skin page
  (`apps/app-frontend/src/pages/Skins.vue`, `EditSkinModal.vue`). The first
  layer is the unframed figure with one place to act under it (Edit
  appearance, or Revert / Apply while a library skin is tried on) beside a
  grid of skin pictures whose first cell adds a skin. The second layer
  replaces the grid while the figure follows the draft. An earlier version
  stacked section labels, a current-skin row, a library row, a view reset
  key and cape/elytra segments on one level; the maintainer judged it no
  clearer than before, which is why only the figure, its one action and the
  pictures stay on the first layer. Everything else moved into ⋯ menus or
  the second layer.
- Texture, arm model and cape changes live in one draft. Choosing a picture,
  a file, an arm model or a cape updates only the local preview. Apply sends
  the changes that actually differ; Cancel or Escape restores the previous
  preview and returns focus to Edit appearance. Offline accounts edit their
  `SkinChoice` source in that second layer and do not call Mojang.
  Third-party accounts stay read-only.
- A profile snapshot is shared by the service for one account, for 60 seconds.
  Concurrent reads of that account share one GET. A manual refresh and the
  post-write confirmation bypass freshness and still honor authentication and
  rate-limit cooldowns. A write response may fill the snapshot, but it is not a
  completed read and cannot confirm the write. One confirmation read happens
  after 11 seconds and is not repeated automatically. It does not prove the
  change has propagated.
- A 401 cools down for 60 seconds for that token fingerprint. A new token clears
  that cooldown. A 429 uses Retry-After when the value is a delay in seconds or
  an HTTP date, clamped to at most 24 hours, otherwise 60 seconds. A new token
  does not clear a 429. Network failures, 5xx and rate limits may keep showing
  the previous profile; a confirmation read must not treat that previous profile
  as success.
- Textures are fetched without a bearer token and reused by normalized HTTPS URL,
  in memory and as PNG files under `cache/skin-textures/`. Removing an account
  drops its profile slot so a later sign-in cannot observe the previous snapshot.

## Consequences

- Positive: preview, wardrobe and confirmation share one profile read; two
  accounts that cite the same texture download it once; a failed refresh does not
  blank a picture that is already on screen.
- Negative / trade-offs: the 60-second snapshot and the single 11-second
  confirmation can lag Mojang. Refresh is explicit. Disk texture files expire and
  are capped; a full or read-only cache directory still shows the downloaded
  bytes for the current process.
- Follow-ups: a default-skin group under the library (backlog).

Shipped: the two-layer look area in `lumilio-ui::wardrobe`; the profile slots in
`service/appearance.rs` and the texture cache in `skin/cache.rs`, with
regression tests for coalesced reads, account isolation, re-sign-in, cooldowns,
library edits without profile reads and the disk cap. The maintainer checked a
real Microsoft skin and cape change and an offline skin in game.
