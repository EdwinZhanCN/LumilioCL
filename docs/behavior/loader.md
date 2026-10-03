# Loader behavior

- Launchable loaders: vanilla, Fabric, Quilt, Forge and NeoForge (plan 0025).
- A loader instance's release id is
  `<loader>-loader-<loader version>-<game version>`; without a loader version
  it has none and the launch reports that.
- Fabric and Quilt publish a profile per (game version, loader version): a
  manifest inheriting the vanilla release and adding libraries and a main
  class. Its address is built from percent-encoded path segments, so an odd
  game version cannot change the address.
- Before use a profile is normalized: its id becomes ours and its parent the
  game version, whatever the publisher named them.
- Launching resolves the profile with its vanilla parent (from disk or the
  catalog) into one manifest, installs it like any release (libraries with a
  maven `name` and repository `url` need no checksum), and stores it fully
  resolved under `versions/<release id>/`. The client jar is stored under the
  loader's id too. Later launches read that manifest and need no network.
- Version lists: the newest stable version is recommended; without a stable one,
  the newest. A version is stable when the publisher says so, or (Quilt, which
  does not) when it carries no pre-release tag.
- Choosing a version (plan 0024): Fabric and Quilt list their versions per
  game version as above. Forge's list comes from its official
  `maven-metadata.json` (builds per game version, oldest first, named
  `<game>-<build>[-<branch>]`); a build is stable when `promotions_slim.json`
  names it `<game>-recommended`, and the list stands without promotions.
  NeoForge's list comes from its maven API (all builds, oldest first); the game
  version is read from the build number (`21.1.77` → `1.21.1`, `26.3.0.31` →
  `26.3`, `26.1.2.5` → `26.1.2`, `…+pre-2` → `<game>-pre-2`, `0.<snapshot>.n`
  → the snapshot), numbers that fit no scheme are skipped, and 1.20.1 adds the
  builds published under the old `net.neoforged:forge` coordinates. NeoForge
  builds tagged `-beta`/`-alpha`/`-rc` are not stable. Every list is newest
  first.
- Game versions for choosing are the catalog's entries, newest first; the
  catalog calls pre-releases and release candidates snapshots, so their channel
  is read from the id (`-pre`, `Pre-Release`, `-rc`, `Release Candidate`).
- Forge and NeoForge install from their official installer (plan 0025). The
  installer is downloaded once to `versions/<id>/<id>-installer.jar` and kept;
  its `version.json` is the loader profile (normalized and resolved with the
  vanilla parent like Fabric's), so the normal install fetches its libraries.
  Then, while the patched client (`PATCHED`, checked against `PATCHED_SHA` when
  given) is missing, the client-side processors of `install_profile.json` run in
  order on a Java that satisfies the release: their tool libraries are fetched
  (or taken from the installer's `maven/` folder), data values are filled in
  (`[artifact]` → library path, `'text'` → literal, `/path` → extracted from
  the installer), a processor whose promised outputs already match is skipped,
  and every output's SHA-1 is checked after it runs. Server-only processors are
  ignored. The pre-1.13 installer format is refused (`LegacyInstaller`). A
  damaged patched client heals on the next launch because the check runs every
  time.
- A library whose download address is explicitly empty (Forge's patched
  client in `version.json`) is made by the installer: it is on the classpath
  but never downloaded or planned; the processors produce it.

