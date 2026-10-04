# LumilioCL — Agent Work Manual

LumilioCL is a Minecraft launcher in **Rust + GPUI + gpui-component**. The goal is a good
launcher; the documents below serve that goal and never outrank it.

## Invariants

1. **core does not depend on UI.** `lumilio-core` never depends on gpui / gpui-component. Domain
   logic stays independently testable. This is the insurance against UI framework churn.
2. **`3rd-party/` is read-only**: never modify, move or delete anything under it.
3. **Adapted code keeps its attribution.** Code taken or adapted from upstream carries a comment
   naming the source path and its license notice (ADR 0011, 0022). Respect files under a
   different license, never copy Modrinth branding, and adapt to Rust and our crate boundaries
   instead of porting mechanically.
4. **Closed verification loop.** Every change passes all four commands below before handoff.
   Fix failures immediately; "commit now, fix later" is forbidden.

## Verification commands (in this order)

```sh
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

## Where the facts are

- **What the launcher does today**: the generated user paths in `docs/ia/paths/`, then the code.
- **How others do it**: read the upstream source directly.
  - `3rd-party/modrinth`: Modrinth App. `packages/app-lib` is a Rust launcher backend, the first
    place to look for how something is built (auth, Java runtimes, instances, modpacks,
    processes); `packages/daedalus` covers version metadata.
  - `3rd-party/HMCL`: HMCL (Java). The first place to look for feature breadth, platform quirks
    and edge cases.
- **Why things are the way they are**: `.agents/decisions/`. **What is in flight**: `.agents/plans/`.
  **What is wanted but not planned**: `.agents/plans/backlog.md`.
- **How the UI looks, moves and speaks**: `docs/design-language.md` and `docs/design-patterns.md`.

Don't write documents that restate the code or upstream; they go stale (ADR 0021).

## Structure

- Rust stable (pinned by `rust-toolchain.toml`), edition 2024.
- Dependency direction `lumilio-app → lumilio-ui → lumilio-core`. Core owns launcher state,
  metadata, downloads, auth and instance operations. UI owns pages and GPUI state. App owns
  startup, configuration, logging and composition.
- Async: core uses tokio; UI bridges via `cx.spawn`/channels. **No blocking I/O on the UI thread.**
- File layout: a module does one thing. Past about 800 lines of non-test code, split a file into a
  directory module (`name/mod.rs` for the public surface and shared types, one file per concern).
  Unit tests live in `tests.rs` or `tests/<theme>.rs` beside the code (`#[cfg(test)] mod tests;`),
  not inline. Sibling modules use explicit `use` paths and `pub(super)`. Only the real public
  surface is `pub`, re-exported from `mod.rs` so `lumilio_core::X` / `lumilio_ui::X` paths do
  not change.

## UI

- Before UI work, load the `gpui` and `gpui-component` skills. Prefer existing gpui-component
  components over custom ones.
- Buttons, switches, tabs, segments and tags come from `lumilio-ui` (`key::Key`, `controls`,
  `kit`), drawn per design-language §12. Don't use gpui-component's `Button`/`Switch`/`TabBar`
  in pages.
- A built user path is declared by a one-line comment at its code:
  `// ia[page]: 操作 | 层 / 组件 | 结果与反馈 [| 备注]` (ADR 0019, 0021; skill `lumilio-ia-paths`).
  `cargo run -p lumilio-docgen -- ia` regenerates `docs/ia/paths/`, and `cargo test` fails
  while it is stale.

## Plans and decisions

- Multi-step or multi-session work gets a plan in `.agents/plans/<slug>.md`, and the plan
  exists only while the work is active. When it finishes, condense it into the next decision
  record if it made a choice worth explaining, and delete it either way (skill
  `lumilio-exec-plan`). At session start, read the `in_progress` plans.
- Code cites `ADR NNNN`, never a plan. Older "plan NNNN" citations resolve through
  `.agents/decisions/plan-history.md`.
- An ADR records a decision; it is not a permission gate. Nothing needs an ADR before it is built.

## Skills (`.agents/skills/`)

- `lumilio-select-checks`: map a diff to the narrowest checks, then run the full loop.
- `lumilio-exec-plan`: create, continue and close plans.
- `lumilio-write-a-test`: choose the core/UI/app test boundary and prove a guard can fail.
- `lumilio-ia-paths`: declare user paths and regenerate `docs/ia/paths/`.
- `lumilio-motion-design`: apply the design language to UI, motion, copy or world-scene changes.

Escaped failures are written up in `.agents/postmortems/`.

## Hardening

When a mistake recurs, fix the cause with the cheapest mechanism that stops it: a test, then a
lint or check, then a skill step. Add a line here only if you can name the real failure it
prevents. Remove rules that no longer prevent anything.
