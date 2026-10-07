You are filling in the English catalog of LumilioCL, a Minecraft launcher.

Files:
- Source, do not edit: crates/lumilio-ui/i18n/zh-CN/lumilio-ui.ftl
- Target, edit only this: crates/lumilio-ui/i18n/en/lumilio-ui.ftl

Task: for every message id in the source that the target lacks, add the English
message to the target. Put it in the same `##` section and the same order as in
the source; create the section heading in English if it is missing. Do not
change, reorder or delete messages already in the target. Do not edit any other
file.

Fluent rules:
- Use exactly the arguments the Chinese message uses: every `{ $name }` in the
  Chinese appears in the English, and no new ones.
- Wherever a number argument counts things, choose the English plural with a
  select, even if the Chinese has none:
      library-count = { $count ->
          [one] { $count } game
         *[other] { $count } games
      }
- Leading and trailing spaces are dropped by Fluent; write a value that needs
  them as a string literal, e.g. `{", "}`.
- A `#` comment above a Chinese message explains it; write the comment above the
  English message in English.

Voice (docs/design-language.md §8):
- Write what an English launcher would say; translate the meaning, not the
  words. Calm and direct, second person implied, no exclamation marks.
- Sentence case for everything, including titles, tabs, keys and menu items.
  Proper nouns keep their capitals: Minecraft, Java, Fabric, Forge, NeoForge,
  Quilt, Modrinth, CurseForge, Microsoft, Xbox, Discord, Litematica.
- Keep "…" (one character) where the Chinese has it.
- Keep the Chinese words "简体中文" as they are.
- Terms: 游戏 (an installed instance) = game; 游戏库 = Library; 发现 = Discover;
  动态 = Activity; 整合包 = modpack; 模组 = mod; 资源包 = resource pack;
  光影 = shader; 投影 = schematic; 世界 = world; 存档 = save; 加载器 = loader;
  启动 = launch, but a key that starts the game says "Play"; 技术详情 =
  Technical details.
- English is wider than Chinese; prefer the shorter of two equally clear
  wordings for keys, tabs and segments.

When done, run `cargo test -p lumilio-ui i18n` and fix the English catalog until
it passes. Reply with one line: how many messages you added, and whether the
tests passed.
