# 0042 — Update from verified stable release assets

- Status: accepted
- Date: 2026-10-10

## Context

Each release publishes a platform archive or installer and a `SHA256SUMS.txt` manifest. The operating systems and package formats have different ownership and replacement rules: the Linux user installer owns a per-user executable, `.deb` belongs to the package manager, and the Windows ZIP has no fixed install directory.

## Decision

The launcher checks stable release assets through the Worker, falls back to GitHub, downloads the matching asset, and verifies its SHA-256 against the HTTPS manifest before offering restart-to-update. Automatic checks default on at startup and run hourly; users can disable them or check manually. Apply updates only to the macOS app bundle, the Windows Inno installer, and the Linux `install.sh` location. Debug builds, Windows portable ZIPs, Linux `.deb` packages, and unsupported install locations do not update themselves.

The manifest remains unsigned; HTTPS is the trust boundary. The first implementation has no rollback flow.

## Consequences

- Positive: stable updates need no separate metadata service, and interrupted or corrupt downloads are never offered for installation.
- Negative / trade-offs: release asset names and the manifest format are compatibility surfaces; unsupported package types require their normal installer or package manager.
- Follow-ups: validate each native package in release CI and have the maintainer test a real 0.1.0-to-next-version update.

Shipped: update client, platform installers, settings, status UI, release build variants and package documentation in the 0.1.0 work.
