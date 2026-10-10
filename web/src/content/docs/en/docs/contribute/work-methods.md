---
title: Choose a work method
description: Use a clear procedure for manual work, agent work, or joint work.
---

The same acceptance conditions apply to all three work methods.
Each person or Coding Agent that does project work uses the name `Project Maintainer`.

## Work without a Coding Agent

1. Read `AGENTS.md` and the relevant JSON plans.
2. Examine the current behavior.
3. Define the change and its acceptance conditions.
4. Make the change.
5. Run the applicable checks.
6. Review the result.
7. Prepare the handoff report.

Use [Change and review](/en/docs/contribute/change-and-review/) for the complete procedure.

## Work as a Coding Agent

1. Read `AGENTS.md` before you edit files.
2. Read relevant `in_progress` JSON plans.
3. Examine Git status.
4. Read each skill that applies to the task.
5. Examine the code and locked dependencies.
6. Complete the authorized work.
7. Run the applicable checks.
8. Update the plan with the results.
9. Report the change and any remaining limits.

Unless the task authorizes their removal, keep existing changes.
Do not replace evidence with an assumption.
Report a failed command and its cause.
Ask for missing information when it prevents correct work.

Repository access does not authorize publication or release operations.
Follow the task instructions for commits, pushes, deployments, and external messages.

## Work with a Coding Agent

Before work starts:

1. State the intended result.
2. State the acceptance conditions.
3. Identify the allowed file scope.
4. Identify any required manual checks.
5. State which external actions are authorized.

During work:

1. Review the Coding Agent's findings.
2. Correct missing or incorrect requirements.
3. Keep decisions in the JSON plan when a plan is necessary.

Before handoff:

1. Read the diff.
2. Check the command results.
3. Examine UI changes in the application or browser.
4. Record any manual check that remains.

Successful automated checks do not prove visual quality.
Do not mark a plan completed while a required acceptance check remains.
