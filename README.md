# LumilioCL

LumilioCL is a new Minecraft launcher built in Rust with GPUI and
gpui-component. HMCL and Modrinth App are its read-only upstream references
(`3rd-party/`, not committed); code adapted from them keeps its attribution.

## Workspace

- `lumilio-core`: UI-independent launcher domain logic.
- `lumilio-ui`: GPUI views and interaction state.
- `lumilio-app`: desktop process startup and composition.
- `lumilio-schematic-render`: GPUI-independent native schematic rendering.

The native rendering dependencies are maintained as editable Rust source forks
under [`forks/`](forks/README.md), outside the launcher workspace membership.

## Run locally

```sh
cargo run -p lumilio-app
```

Checks are [`just`](https://github.com/casey/just) recipes: `just check` runs build, tests,
clippy and rustfmt, the same as CI; `just` lists the rest.

The app opens the data folder and shows the real launcher. If the folder cannot
be opened (for example another instance holds the lock) it prints the reason and
exits. Use `LUMILIO_HOME=/tmp/<isolated-folder>` for a disposable data folder.

What the launcher can do is generated from the code into
[`docs/ia/paths/`](docs/ia/paths/README.md); work in flight is in
[`.agents/plans/`](.agents/plans/README.md).

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

## License

LumilioCL is licensed under the GNU Affero General Public License, version 3 only
(`AGPL-3.0-only`); see [`LICENSE`](LICENSE).

Some code is adapted from two upstream launchers, and each such file or function names its
source and license in a comment:

- [HMCL](https://github.com/HMCL-dev/HMCL), GPL-3.0-or-later (ADR 0011).
- [Modrinth App](https://github.com/modrinth/code), GPL-3.0-only (ADR 0022). Modrinth's branding
  is not used.

Bundled fonts and icons are listed with their licenses in
[`crates/lumilio-ui/assets/ATTRIBUTIONS.md`](crates/lumilio-ui/assets/ATTRIBUTIONS.md).
