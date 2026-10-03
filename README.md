# LumilioCL

LumilioCL is a new Minecraft launcher built in Rust with GPUI and
gpui-component. The project uses HMCL only as a read-only behavioral reference;
the implementation and product architecture are original to LumilioCL.

## Workspace

- `lumilio-core`: UI-independent launcher domain logic.
- `lumilio-ui`: GPUI views and interaction state.
- `lumilio-app`: desktop process startup and composition.

## Run locally

```sh
cargo run -p lumilio-app
```

The app opens the data folder and shows the real launcher. If the folder cannot
be opened (for example another instance holds the lock) it prints the reason and
exits. Use `LUMILIO_HOME=/tmp/<isolated-folder>` for a disposable data folder.

See [`docs/roadmap.md`](docs/roadmap.md) for status and
[`.agents/plans/history.md`](.agents/plans/history.md) for what has landed.

### Review hooks

All of these affect only the current run:

- `LUMILIO_PAGE` opens a page directly, e.g. `library:2`, `discover:1`, `activity`,
  `instance:1:4:1` (id, section, sub-tab; zero-based).
- `LUMILIO_INSTANCE=<id>` (with `LUMILIO_INSTANCE_TAB`, `LUMILIO_INSTANCE_GROUP`) and
  `LUMILIO_PROJECT=mod/sodium` open a game or a project detail.
- `LUMILIO_REVIEW_APPEARANCE=light|dark`, `LUMILIO_REVIEW_MIN=1` (720×480),
  `LUMILIO_REVIEW_STILL=1` (reduced motion).

## Design

UI look, motion, and copy follow [`docs/design-language.md`](docs/design-language.md).
