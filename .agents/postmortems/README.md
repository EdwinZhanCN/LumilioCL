# Postmortems

Postmortems capture escaped failures: a defect reached a user, release, or
protected branch because the existing process or tests missed it. The point is
the failed safety net and the durable guardrail, not a chronological retelling
of the fix.

Files are `NNNN-kebab-case-title.md` and contain:

```markdown
# Postmortem NNNN: <title>

## Executive summary
## What broke
## Why every net missed it
## Guardrails added
```

`## Guardrails added` must link the actual rule, skill, regression test, or
verification command that prevents recurrence. An empty directory is a good
state; do not create speculative postmortems.
