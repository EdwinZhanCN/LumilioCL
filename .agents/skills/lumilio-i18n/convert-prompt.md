You are moving the hardcoded Chinese text of some LumilioCL source files into
the Simplified Chinese language catalog. LumilioCL is a Minecraft launcher in
Rust + GPUI. Read `.agents/skills/lumilio-i18n/SKILL.md` first; it is the
rulebook.

Files to convert: FILES

Edit only those files and `crates/lumilio-ui/i18n/zh-CN/lumilio-ui.ftl`. Do not
touch the English catalog, tests, `hardcoded.txt`, or any other file.

For every string literal with Chinese in those files (skip comments, and skip
an inline `#[cfg(test)] mod tests` at the end of a file):

1. Add a message to the Chinese catalog under a `## <page>` section (create it
   before `## 游戏库与发现的搜索和筛选` if missing), with an id
   `page-part-meaning` in lowercase with hyphens. Reuse an existing message
   when the catalog already has the same words with the same meaning,
   especially `common-*`, `route-*` and `environment-*`; search the catalog
   before adding.
2. Replace the literal with `tr!("id")` (a `&'static str`) or, when the text
   has variables, `tr!("id", name = value)` (a `String`). The Fluent message
   uses `{ $name }` for each argument. Use `crate::tr!` / `use crate::tr;` in
   lumilio-ui and `use lumilio_ui::tr;` in lumilio-app. A `const` array of
   labels becomes a function returning `tr_all![...]`. A `const fn` that now
   calls `tr!` stops being `const`.
3. One sentence is one message: turn `format!("已导入「{}」", name)` into a
   message with a `{ $name }` argument, never into pieces joined in Rust.
   Lists joined with `、` use `tr!("common-list-separator")`.
4. A number that counts things is an argument (`count = n`) so English can
   choose a plural; an identifier such as an exit code is passed as a string.
5. Keep code behaviour the same. If a value must stay a literal for a reason
   (matching text from a file the game writes, a protocol value), leave it and
   add `// i18n-exempt: <why>` at the end of that line.

When every file is done, run `cargo check -p lumilio-app` once and fix only
what it reports. Do not run tests, clippy or `just` recipes; the maintainer
runs those.

Reply with one line: how many literals you moved, how many messages you
added, and whether `cargo check` passed.
