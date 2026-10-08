---
name: lumilio-i18n
description: Use when adding or changing text a person reads in lumilio-ui or
  lumilio-app, or when moving a page's hardcoded Chinese into the language
  catalogs — message ids, `tr!` / `tr_all!`, arguments and plurals, the
  hardcoded-Chinese list, and filling the English catalog.
---

# Launcher Text In Catalogs

The launcher's own words live in Fluent catalogs, not in Rust:
`crates/lumilio-ui/i18n/zh-CN/lumilio-ui.ftl` (the fallback, always complete)
and `crates/lumilio-ui/i18n/en/lumilio-ui.ftl`. Game logs, crash reports and
text from content sources never go through them.

## In code

- `tr!("settings-title")` is a `&'static str`; it goes wherever a literal went.
- `tr!("settings-mirrors-count", count = n)` is a `String`.
- `tr_all!["a", "b", "c"]` is a `&'static [&'static str]` for segments and
  choices; a `const` array of labels becomes a function returning it.
- Ids are `page-part-meaning` with hyphens (Fluent ids cannot hold dots).
  Reuse `common-*` for words every page says (无, 编辑, 添加).
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

## Moving a page

1. Replace the page's Chinese literals with `tr!` and add each message to the
   Chinese catalog under a `## <page>` section, in reading order.
2. Fill the English catalog: either write it, or hand it to DeepSeek through
   the maintainer's `dsh` (it edits the English file and runs the i18n tests):

   ```bash
   dsh --profile headless "$(cat .agents/skills/lumilio-i18n/translate-prompt.md)"
   ```

   Then read the English diff yourself. Check plurals, sentence case, the term
   list in the prompt, and that keys stay short. `dsh` is not in CI; the
   catalogs it writes are reviewed like any other change.
3. `just hardcoded-chinese` so `crates/lumilio-ui/src/i18n/hardcoded.txt`
   shrinks, then `cargo test -p lumilio-ui i18n` and
   `cargo test -p lumilio-ui --test english`.
4. Tests in the unit-test binary assert Chinese; switching the language is
   process-wide, so English assertions go in `crates/lumilio-ui/tests/english.rs`.

## Failure modes

- **A word left in Rust.** `hardcoded_chinese_only_shrinks` fails and names the
  file; move it into the catalog instead of regenerating the list.
- **An English entry with different arguments.** The catalog test fails; the
  English must use the Chinese message's arguments.
- **A count without a plural.** "1 games". Every counted noun gets a select.
