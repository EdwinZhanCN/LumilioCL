# 0024 — Third-party accounts (authlib-injector), offline skins and a built-in LittleSkin

- Status: accepted
- Date: 2026-10-04

## Context

ADR 0018 deferred offline skins until third-party authentication was accepted, because both need the
authlib-injector agent in the game's JVM and offline skins also need a skin server. The maintainer
wants LittleSkin supported, built in, and asked for HMCL's business logic to be the truth. HMCL
(`HMCLCore/.../auth/yggdrasil`, `authlibinjector`, `offline`; GPL-3.0-or-later, ADR 0011) is the reference;
Modrinth App has no equivalent.

## Decision

1. **Accounts.** A third-party account is an `AccountKind::ThirdParty` entry: character name and id,
   its server's API root and the login name. The key is `ali:<profile id>@<API root>`, so the same
   character id on two servers never clashes. Secrets (client token, access token, the server's
   `user.properties`) live only in the system credential store, like Microsoft's (ADR 0020).
2. **Servers.** LittleSkin (`https://littleskin.cn/api/yggdrasil/`) is built in and cannot be removed.
   Others are added by address: `https://` is assumed, the `x-authlib-injector-api-location` header
   is followed, and the person sees the server's own name (and an HTTP warning) before it is kept.
   Removing a server removes the accounts signed in on it, with their secrets.
3. **Session.** Sign-in is `authenticate` with a random client token; a single character is chosen
   for the person, several ask (the session waits in memory only). A launch `validate`s the stored
   token and renews it when refused; `ForbiddenOperationException` on renewal, another character, or a
   missing name means "sign in again". A network failure never falls back to offline (ADR 0020) and
   never marks the account as needing sign-in.
4. **The agent.** authlib-injector is fetched the first time it is needed from the official index
   through the configured source chain (mirrors apply), and kept only when its SHA-256 matches the
   index and its manifest names it. HMCL ships the jar; we do not put it in the repository. A newer
   build is looked for once per run, in the background. The JVM gets
   `-javaagent:<jar>=<API root>`, `-Dauthlibinjector.side=client` and, for a real server, the server's
   metadata as `-Dauthlibinjector.yggdrasil.prefetched`.
5. **Offline skins.** An offline account may have a `SkinChoice`: local files (model, skin, cape),
   LittleSkin (`https://littleskin.cn/csl`), or a CustomSkinLoader API. No choice is the game's default
   skin and starts nothing extra. With a choice, the launch loads the skin (a failure is told in the
   game log, the game still starts), starts a Yggdrasil server on this machine for that one player
   with HMCL's routes, texture hashing and SHA1withRSA signatures, and points the agent at it. The
   server lives exactly as long as the launch's `AuthSession`. The RSA key is made once and kept in the
   launcher's folder (HMCL makes one per run) so a launch never waits for key generation.
6. **Dependencies.** `rsa` 0.9 with the legacy `sha1` 0.10 (digest versions must match), `base64` and
   `getrandom`; `image` gains PNG and JPEG decoding (screenshots and skins); tokio gains `net`.

## Consequences

- Positive: LittleSkin and any authlib-injector server work; offline players can wear a skin; every
  step is covered by scripted-server or real-TCP tests, including the route answers and a verifiable
  signature.
- Negative / trade-offs: the first launch that needs the agent downloads it; a skin needs the network
  at launch (LittleSkin / CSL); the server only speaks http on localhost, as HMCL's does.
- Not built: uploading skins to a server, HMCL's built-in Steve/Alex/… defaults (they need Mojang's
  pictures), dragging an `authlib-injector:` link in, and a 3D preview of the skin.
- Shipped: `lumilio-core` `yggdrasil`, `injector`, `skin` and service `third_party` / `skins`;
  `lumilio-ui` `third_party_login`, `auth_servers`, `skin_dialog` and the Accounts page; `lumilio-app`
  wiring. Needs the maintainer: a real LittleSkin sign-in and a launch that shows the skin.
