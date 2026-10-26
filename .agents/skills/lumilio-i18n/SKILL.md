---
name: lumilio-i18n
description: Use when adding or changing text a person reads in lumilio-ui or
  lumilio-app, or when moving a page's hardcoded Chinese into the language
  catalogs — message ids, `tr!` / `tr_all!`, arguments and plurals, the
  hardcoded-Chinese list, filling the English catalog, and revising wording
  using the plain-language guidance.
---

# Launcher Text In Catalogs

The launcher's own words live in Fluent catalogs, not in Rust:
`crates/lumilio-ui/i18n/zh-CN/lumilio-ui.ftl` (the fallback, always complete)
and `crates/lumilio-ui/i18n/en/lumilio-ui.ftl`. Game logs, crash reports and
text from content sources never go through them.

Use the relevant parts of `docs/plain-language.md` for content, voice and form.
Adapt wording and presentation to the feature. For substantial rewrites or
consequential choices, `plain-review-prompt.md` offers a rewrite and a cold read;
ordinary label edits do not require that workflow.

## In code

- `tr!("settings-title")` is a `&'static str`; it goes wherever a literal went.
- `tr!("settings-mirrors-count", count = n)` is a `String`.
- `tr_all!["a", "b", "c"]` is a `&'static [&'static str]` for segments and
  choices; a `const` array of labels becomes a function returning it.
- Ids are `page-part-meaning` with hyphens (Fluent ids cannot hold dots).
  Reuse `common-*` for words every page says (无, 编辑, 添加).
- A hint's id ends in `-help` or `-note`: a ⓘ, a key's tooltip, a line under
  a field, an empty state's line. The existing `hints_fit_one_line` check
  limits these ids as described in plain-language §6; it does not measure layout.
- The arguments at the call must equal those the Chinese message uses; `tr!`
  fails to compile otherwise. Design a sentence so both languages need the same
  arguments.
- Never join translated pieces into a sentence; one sentence is one message.
  A list separator is `tr!("common-list-separator")`.
- The system's own names come from `platform` (`reveal_label`).
- An id known only at run time (a Modrinth tag) goes through
  `i18n::lookup("tag-…")` with a fallback; it is not checked at compile time.
- Formatting that differs by language beyond words (digit grouping: 万 / K)
  branches on `i18n::locale()`. A line that is Chinese on purpose ends with
  `// i18n-exempt: <why>` and is not counted as hardcoded.
- `lumilio-app` uses `lumilio_ui::tr!` with the same catalogs
  (`crates/lumilio-app/i18n.toml`).
- Words kept in an entity's state (an input's placeholder, a dropdown's items)
  do not follow a switch by redrawing. Remember `i18n::generation()` when they
  are taken and take them again when it changes, keeping what is chosen
  (`LiveControls::relabel`).

## Outside the UI

- `lumilio-core` never words a sentence: it returns types (a task label is an
  action and a subject, a reclaim reason is an enum) and the UI maps them.
  Its error `Display` text is English and technical.
- A plugin owns its words: manifest, setting and tab text are `Words`
  (Chinese and English), and text a plugin makes at run time follows
  `HostContext::locale()`. Keep them in the plugin's `src/text.rs`. Setting
  choice options are stored values and are never translated.

## Moving a page

1. Replace the page's Chinese literals with `tr!` and add each message to the
   Chinese catalog under a `## <page>` section, in reading order.
2. Fill the English catalog using the requirements in
   `.agents/skills/lumilio-i18n/translate-prompt.md`. Review the English diff:
   check plurals, sentence case, the term list and short keys. This workflow
   requires no private local tool.
3. `just hardcoded-chinese` so `crates/lumilio-ui/src/i18n/hardcoded.txt`
   shrinks, then `cargo nextest run -p lumilio-ui i18n` and
   `cargo nextest run -p lumilio-ui --test english`.
4. Tests in the unit-test binary assert Chinese; switching the language is
   process-wide, so English assertions go in `crates/lumilio-ui/tests/english.rs`.

## Failure modes

- **A word left in Rust.** `hardcoded_chinese_only_shrinks` fails and names the
  file; move it into the catalog instead of regenerating the list.
- **An English entry with different arguments.** The catalog test fails; the
  English must use the Chinese message's arguments.
- **A count without a plural.** "1 games". Every counted noun gets a select.
- **A long hint.** `hints_fit_one_line` names it. Keep necessary facts reachable
  (plain-language §6). If richer help needs a different surface, change that
  surface and its validation together; do not evade the check by renaming the id
  or growing `long-hints.txt`.
