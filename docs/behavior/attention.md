# Home summary behavior

- **Continue**: the most recently played instance with no error-level problem.
  An instance that cannot launch is never offered. If nothing has been played,
  there is nothing to continue.
- **Recent**: other played instances, newest first, up to a limit. Never-played
  instances are not recent.
- **Needs attention**: instances with any warning or error problem — errors
  first, then most recently played, then id. Info problems (such as "not
  installed yet") do not count. Each entry carries its most severe problem (the
  first reported among equals) and the number of warning-or-worse problems.
