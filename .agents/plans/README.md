# Plans — retained JSON lifecycle

Edit `<slug>.json` only. Stable `id` is independent of filename; preserve plan
and task IDs. `backlog.md` holds unscheduled ideas. Small Issue/PR fixes need no plan.

## Lifecycle

`proposed → in_progress → completed`; `blocked` and `cancelled` record real
interruptions. Completed means work and acceptance finished, never released.
Keep JSON, including cancelled plans. Decisions and lessons go here. Long-lived
cross-plan contracts belong in code/API/design documents with stable references.

## Contract and commands

- [v1 structural schema](../schemas/plan.schema.json)
- [valid proposed example](../schemas/examples/proposed.json)
- Invalid fixtures: [unknown status](../schemas/examples/invalid-status.json),
  [missing acceptance](../schemas/examples/missing-acceptance.json); docgen tests
  must reject both, and exercise duplicate IDs and invalid dependencies.
- `cargo run -p lumilio-docgen -- plans validate`: strict typed v1 parsing plus
  unique plan/task/decision/roadmap IDs, valid dependencies, nonempty acceptance,
  and completion evidence.
- `just plans`: generate `docs/plans/*.md` and `web/src/data/roadmap.generated.json`.
- `just plans-check`: reject missing, edited or orphan generated output.
- `just docs`: plan tests, freshness, existing IA checks and formatting.

Required: `schemaVersion: 1`, stable `id`, `title`, `summary`, `status`,
`visibility`, `scope` (`included`/`excluded`), nonempty `tasks` and `validation`.
Each task has `id`, `title`, `status`, observable `acceptance`. Each validation
entry has `criterion` and optional `result`: null means pending; a result records
evidence, not a command to run. Completed requires terminal tasks, all validation
results and nonempty `outcome`.

Optional `decisions` have `id`, `decision`, `rationale`, `consequences`;
`references` are paths/URLs; `dependencies` are existing plan IDs;
`lessonsLearned` records failures and linked guards; `outcome` states the result.

## Public boundary

`visibility: internal` excludes website projection, not repository readers:
generated Markdown includes internal fields. Never store credentials. Only
`visibility: public` AND `roadmap.enabled: true` enters the website; cancelled
plans are excluded. These selected plans are the website's primary plans, shown
on the independent Roadmap page. The whitelist is roadmap `id`, `planId`, `name`, `lede`,
`version` and plan `status`, ordered by `roadmap.order` then plan ID. No nested
tasks, decisions, references, validation or lessons are copied. `targetVersion`
is a target, never a release or date. Logo positions use three curated IDs.

No standalone ADRs, postmortems, legacy/archive directories, Markdown redirects
or historical mapping tables. Removed material remains available in Git history.
