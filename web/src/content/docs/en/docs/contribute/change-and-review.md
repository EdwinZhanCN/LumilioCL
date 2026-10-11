---
title: Change and review
description: Define acceptance, implement a change, verify it, and report the result.
---

## Define the work

1. Examine Git status.
2. Find the relevant code and user paths.
3. Define acceptance conditions that you can check.
4. Create or update a JSON plan if the work needs multiple steps or sessions.
5. Set the plan status to `in_progress` before implementation.

Use `.agents/schemas/examples/proposed.json` as the plan example.
Read `.agents/plans/README.md` for the plan contract.
Use stable plan and task IDs.
Remove a backlog entry when it becomes a plan.

## Implement the change

Keep each module focused on one responsibility.
Split a module near 800 lines of non-test code.
Put unit tests in a sibling `tests.rs` or `tests/` module.
Keep the public API at the module boundary.

Put launcher text in the language catalogs.
Complete both Chinese and English entries.
Use the `lumilio-i18n` skill for message IDs and arguments.

Declare each implemented user path with an `ia` comment:

```rust
// ia[page]: 操作 | 层 / 组件 | 结果与反馈 [| 备注]
```

After you change these comments, run `just ia`.
Do not edit `docs/ia/paths/` by hand.
Do not declare a planned feature as an implemented path.

For UI changes, read the GPUI skills and `docs/design-language.md`.
Check light, dark, and reduced-motion states.
Record the states that you actually inspected.

## Check the result

1. Run the narrowest applicable check during implementation.
2. Fix each failure before handoff.
3. Run the required handoff recipe from [Set up development](/en/docs/contribute/setup/#select-checks).
4. Perform the required manual checks.

Test behavior at the correct boundary.
Keep domain tests independent of GPUI.
For a regression test, introduce the defect temporarily.
Confirm that the test fails for that defect.
Then remove the defect.
Run the test again.

Do not change a check to hide a failure.
Do not state that future CI will fix a local failure.

## Keep the plan accurate

1. Record validation results.
2. Record decisions with their reasons and consequences.
3. Record failure lessons and the guards that prevent recurrence.
4. Record the outcome after acceptance is complete.
5. Mark the tasks and plan completed.
6. Run `just plans`.
7. Run `just plans-check`.

Keep the JSON file after completion or cancellation.
Use `blocked` when a real condition prevents progress.
Record that condition and the remaining task.
Completion does not mean publication or release.

## Prepare the handoff

Describe the problem and the resulting behavior.
Include a concrete before-and-after example when it helps review.
List the checks that passed.
State any remaining manual checks or known limits.
Use more detail for a complex change.

A pull request must explain the final implementation.
Do not make a reviewer reconstruct the conversation.

When the task authorizes submission:

1. Commit the verified change to its branch.
2. Push that branch to your fork or the authorized repository.
3. Open a pull request against `main`.
4. Add the handoff report to the pull request description.
5. Resolve check failures before merge.

## Branches and merges

`main` is the only long-lived branch, and it is always ready for release.
All changes go into `main` through a pull request, including changes from maintainers.
`main` does not accept direct pushes or force pushes.

Start the branch name with the same type prefix that the commit uses:

| Prefix | Use |
| --- | --- |
| `feat/` | New feature |
| `fix/` | Defect fix |
| `docs/` | Documents, plans, or harness |
| `build/` | Build, checks, or CI |
| `web/` | Website |

The `rust`, `plans`, and `web` checks must pass before merge.
A pull request runs all three checks, also when the change touches only one area.
Squash merge is the default.
After the merge, GitHub deletes the branch.

When a generated file has a conflict, do not merge it manually:

1. Resolve the conflicts in the source files first.
2. For the generated file, accept either side.
3. Run `just plans` for plan output, or `just ia` for user paths.
4. Commit the generated files again.

The generated files are `docs/plans/`, `web/src/data/roadmap.generated.json`, and `docs/ia/paths/`.

A release is a `v*` tag on `main`.
Only maintainers can create release tags.
Create a `release/<major.minor>` branch from a version tag only when that version needs a patch and `main` already contains changes for the next version.

Include source attribution for adapted code.
The project license is `AGPL-3.0-only`.
Read the [license](https://github.com/EdwinZhanCN/LumilioCL/blob/main/LICENSE) before you distribute a changed application.
