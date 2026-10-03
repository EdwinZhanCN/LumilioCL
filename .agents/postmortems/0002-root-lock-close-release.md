# Postmortem 0002: Root lock released only by closing its file

## Executive summary

The root-lock slice reached a release build with close-only release. A later parallel full suite observed RootBusy after explicit service drop. RootLock now unlocks explicitly, and a deterministic descriptor-retention regression test protects that ownership boundary.

## What broke

The immediate service-reopen test failed despite dropping the old service. Retaining a cloned open-file description reproduces the same failure deterministically when explicit unlock is removed. Child inheritance before close-on-exec is a plausible source of such retention in the full suite; that exact spawn race is not yet proven.

## Why every net missed it

The first independent-process tests covered normal process exit and kill, but did not retain an alias of the owner's file description. Earlier full suites passed. Those cases did not prove that closing one handle alone released service ownership.

## Guardrails added

- [RootLock regression](../../crates/lumilio-core/src/root_lock.rs): retain a descriptor, drop ownership, reacquire, then close the old descriptor and prove the new owner remains exclusive. Removing unlock fails at the intended RootBusy assertion.
- [Cross-process integration](../../crates/lumilio-core/tests/root_ownership.rs) continues to cover contention, cooperative exit and process death.
- [Select-checks](../skills/lumilio-select-checks/SKILL.md) requires the full parallel four-step loop; [plan 0015](../plans/history.md) records the escaped failure and restarted loop.
