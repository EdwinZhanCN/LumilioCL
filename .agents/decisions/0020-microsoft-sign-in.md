# 0020 — Microsoft sign-in: device code flow, system credential store, identity by profile id

- Status: accepted
- Date: 2026-10-03

## Context

`L-ACC-02` (online sign-in and recovery) was blocked on four facts: which
client registration signs people in, how the secret tokens are kept, what
identifies an account, and what happens when a token can no longer be used.
The maintainer registered an application in Microsoft Entra ID and supplied its
client (application) id. A client id is public by design (it appears in every
authorization URL); there is no client secret because the application is a
public client. Minecraft's own services additionally require an application to
be approved before they accept its tokens, which the launcher cannot verify
offline: until the registration is approved, the last step of the chain is
refused with a clear message.

## Decision

1. **Flow.** The OAuth 2.0 device authorization grant against the `consumers`
   tenant, scope `XboxLive.signin offline_access`, then the Xbox Live user
   token, the XSTS token for `rp://api.minecraftservices.com/`, the Minecraft
   services login, an ownership check (`entitlements/mcstore`), and the
   profile. No local web server and no redirect URI: the person types a code
   in their own browser, and the launcher polls.
2. **Client id.** Compiled in as a public constant (`MICROSOFT_CLIENT_ID`),
   overridable by the `LUMILIO_MS_CLIENT_ID` environment variable for
   development. It is not a secret and is not treated as one.
3. **Secrets.** The refresh token and the cached Minecraft access token live
   only in the operating system credential store (Keychain, Credential
   Manager, Secret Service), under service `LumilioCL` and the account key. If
   the store is unavailable the sign-in fails with an explanation; secrets are
   never written to `settings.json`, the library, logs or the diagnostics
   bundle, and there is no plaintext fallback. `settings.json` holds only
   public facts: kind, profile name, profile id, and when the cached token
   expires.
4. **Identity.** A Microsoft account is identified by its Minecraft profile id,
   not its name (names can change and may equal an offline name). Account keys
   are the name for an offline account (unchanged) and `msa:<profile id>` for a
   Microsoft account; selection stores the key. The displayed name is refreshed
   whenever the profile is read again.
5. **Using a token.** A launch with a Microsoft account uses the cached
   Minecraft token while it has at least five minutes left, otherwise refreshes
   it through the stored refresh token (which also rotates). When the refresh
   token is rejected the account is marked as needing sign-in and the launch
   stops with a message to sign in again. A network failure never silently
   switches to an offline identity: launching under another identity is a
   choice the person makes, not a fallback. The launch presents the real
   profile name and id, the Minecraft access token, `msa` as the user type, and
   the Xbox user id when known.
6. **Errors are told in words.** Declined, expired code, cancelled, no Xbox
   account, child account (needs family approval), account without Minecraft,
   regions where Xbox Live is unavailable, services refusing the application,
   and network trouble each have their own sentence; the raw cause stays behind
   technical details.

## Consequences

- Positive: no secrets on disk outside the OS store; sign-in works without a
  redirect URI or a browser integration; accounts survive a rename.
- Negative / trade-offs: a new dependency (`keyring`) with per-platform stores;
  Linux needs a running Secret Service; the sign-in cannot be proven end to end
  until Minecraft services approve the registration, so the protocol is tested
  against a scripted server and the final step against the real service is a
  maintainer check.
- Follow-ups: third-party authentication servers (authlib-injector) and skins
  build on the account kinds introduced here; an explicit "play offline once"
  choice for the no-network case is left for a later decision.

## Shipped

The protocol client, credential store, account keys, token refresh, launch
session and the sign-in dialog shipped with this decision. On 2026-10-07 the
registration was approved by Minecraft services and the maintainer signed in
with a real account, confirming the whole chain end to end.
