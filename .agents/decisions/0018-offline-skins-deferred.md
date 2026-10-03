# 0018 — Offline account skins are deferred

- Status: proposed
- Date: 2026-10-03

## Context

An offline account has no skin service: the game shows a default skin chosen by the profile id. Showing a chosen skin requires a local authentication/skin server and the third-party authlib-injector agent in the game's Java arguments. `docs/ia/accounts.md` marks third-party authentication as out of scope (⏸), and the reference notes "skin capability undecided".

## Decision

Do not build offline skins now. They need the third-party authentication scope to be accepted first, a decision on bundling or downloading the agent (and its licence), and a design for the local skin server. Until then the Accounts page does not offer a skin control.

## Consequences

- Positive: no unreviewed third-party code in the launch path.
- Negative / trade-offs: offline players keep the default skin.
- Follow-ups: revisit together with third-party authentication.
