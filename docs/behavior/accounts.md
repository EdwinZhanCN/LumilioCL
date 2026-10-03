# Accounts and Microsoft sign-in behavior

Decisions: ADR 0013. Offline accounts: `settings.md`, `launcher-spine.md`.

- An account is an offline profile or a Microsoft sign-in. `settings.json`
  holds public facts only: name, profile id, kind, and whether the stored
  sign-in needs renewing. Selection and removal name an account by its key:
  the name for an offline account, `msa:<profile id>` for Microsoft.
- Sign-in (device code): ask for a code, show it with the address, poll at the
  service's interval (adding to it on "slow down"), and stop when the person
  finishes, declines, the code runs out, or cancels. Then the Xbox Live user
  token, the XSTS token for Minecraft's services, the services login and the
  profile. A profile that does not exist means the account does not own
  Minecraft: Java Edition.
- The refresh token and the Minecraft access token (with its expiry) are kept
  only in the system credential store, under service `LumilioCL` and the
  account key. When the store does not work, the sign-in fails before any code
  is shown; nothing is ever written to a file instead.
- Signing in again, or after a rename, finds the account by profile id and
  updates its name; it never adds a second one. An id already used by an
  offline account is refused.
- A launch uses the cached Minecraft token while it has at least five minutes
  left, otherwise refreshes it (the refresh token rotates and is saved first).
  A rejected refresh token, a missing stored sign-in, or a different profile
  than the one the account was added for marks the account as needing a
  sign-in and stops the launch; the game never starts as someone else. A
  network failure stops the launch with its own message and does not mark the
  account. Refresh is one at a time.
- Refreshing from the account's menu does the same and clears an earlier mark.
- Removing a Microsoft account deletes its stored sign-in; an unknown account
  changes nothing. Tokens appear in no log, debug output or diagnostics bundle.
- Failures have their own sentence: declined, expired code, cancelled, no
  Xbox profile, child account, Xbox unavailable in the region, adult
  verification, no Minecraft, services refusing the application, credential
  store unavailable, network, unexpected answer.
