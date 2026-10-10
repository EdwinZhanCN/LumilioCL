---
title: WASM plugins
description: Understand the current plugin limits before you start plugin work.
---

## Current status

Installable third-party WASM plugins are not available.
The current launcher contains built-in native plugins.
Do not use the native plugin API as a published WASM contract.

Read the [WASM plugin plan](https://github.com/EdwinZhanCN/LumilioCL/blob/main/.agents/plans/wasm-plugin-registry.json) for proposed work.
A target version is not a release commitment.

## Future documentation

This section will contain the plugin contract when implementation and acceptance are complete.
The documentation will cover:

- The manifest and compatibility rules.
- Host functions and plugin permissions.
- Build, test, and installation procedures.
- Error, timeout, and cancellation behavior.
- Package verification and distribution.

Use the [contribution rules](/en/docs/contribute/) for current project work.
