---
title: Contribution rules
description: Apply the same project rules to people, Coding Agents, and joint work.
---

Every Project Maintainer uses these rules.
They apply to people, Coding Agents, and people who work with Coding Agents.

## Start a change

1. Read this page.
2. [Set up development](/en/docs/contribute/setup/).
3. [Choose a work method](/en/docs/contribute/work-methods/).
4. Follow [Change and review](/en/docs/contribute/change-and-review/).

Read [AGENTS.md](https://github.com/EdwinZhanCN/LumilioCL/blob/main/AGENTS.md) before you change files.
Use the commands in `justfile` for verification.
The repository files contain the current execution instructions.

## Keep the crate boundaries

The main dependency direction is `lumilio-app → lumilio-ui → lumilio-core`.

| Crate | Responsibility |
| --- | --- |
| `lumilio-core` | Launcher data and operations; no GPUI dependency |
| `lumilio-ui` | Pages, controls, and UI state |
| `lumilio-app` | Startup and application composition |

Keep domain logic independent of the UI.
Do not do blocking I/O on the UI thread.
Use existing controls before you add a new control.
Check UI APIs against the locked dependency source.

## Use source evidence

Read the code for current behavior.
Use `docs/ia/paths/` to find implemented user paths.
Read published upstream source when you compare other projects.
Do not require a private local clone or tool.

Keep source attribution when you adapt upstream code.
Keep all applicable license notices.
Do not copy Modrinth branding.
See [ATTRIBUTIONS.md](https://github.com/EdwinZhanCN/LumilioCL/blob/main/ATTRIBUTIONS.md) for dependency attribution.

## Keep records in JSON

Use `.agents/plans/<slug>.json` for work with multiple steps or sessions.
Small local fixes do not need a plan.
Keep acceptance conditions, decisions, and failure lessons in that JSON plan.
Keep completed and cancelled plans.

After you change a plan, run `just plans`.
Do not edit generated plan Markdown or roadmap data.
Do not create separate ADRs, postmortems, archives, or historical mapping tables.
Use Git history for removed records.

## Use project names

Use `Project Maintainer` for each person or Coding Agent that does project work.
Do not record personal names or machine-specific paths in project instructions.
Use Git-tracked paths or published source URLs.
Keep license attribution and public repository identifiers.
