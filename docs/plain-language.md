# LumilioCL Plain Language

This document decides *what* the launcher's words say and *in what order*.
[Design language §8](design-language.md#8-voice-and-copy) decides the voice and
the form: tone, sentence case, "…", the system's own names. Both apply to every
message in the language catalogs, in both languages, and to labels that a screen
reader reads aloud. Changing a rule here is a design decision: update this file
in the same change and say why.

The rules follow the four principles of ISO 24495-1:2023, *Plain language —
Part 1: Governing principles and guidelines*: readers get what they need, find
it, understand it and can use it. A clause number in brackets shows where a rule
comes from. The standard is copyrighted, so the rules here are our own words;
do not quote the standard or commit it.

## 1. Readers [5.1.2–5.1.4]

- The reader is a player. Most know the game's words (世界, 存档, Mod, 光影,
  服务器). Few know how a launcher works inside.
- They open the launcher to play. They read in passing, one glance per message,
  and they read a failure while they are annoyed.
- A screen reader reads a `-aria` label aloud each time the focus arrives on
  it. A long label costs time on every visit. [3.2]
- When needs conflict, the player's come first: before the wish to explain a
  mechanism, and before the launcher's own records. [3.2, 5.1.6]

## 2. What each kind of message is for [5.1.3]

| Kind | The reader wants to | Write |
| --- | --- | --- |
| Key, menu item | do something | What happens when it is chosen: a verb and its object (导出材料清单). |
| Title | know where they are, or what is asked | The subject or the question. |
| Confirmation body | decide to do it or not | First the consequence, then what is kept. Say plainly what cannot be undone. |
| Setting help | decide to change it or not | What the player will see when it is on, or when the value changes. Not how it works. |
| Failure | know what happened and what to do next | §8: one plain sentence, one action, the raw cause behind **技术详情**. |
| Empty state | know what goes here | What this place holds, and the first action. |
| Placeholder | know what to type | An example value or the format. |
| Screen-reader label | know what the control is | Its name and state. Key help goes into visible help. |

## 3. Choose the content [5.1.6]

- Keep a sentence only if the reader would act or decide differently without
  it.
- Keep the consequence that decides the action: what is lost, what cannot be
  undone, what they must do again (sign in, restart the game). If the message
  does not fit (§6), move other facts out, not this one.
- A question a reader might ask is not a fact they need. Answer it here only
  if the answer changes what they do with this control.
- Leave out how the launcher does it: fallback order, credential store, tokens,
  caches, local servers, file formats. If someone needs them, they go behind
  **技术详情** or into the documentation.
- Some information must be easy to find, but most readers do not need it: what
  data goes to which service, permissions, licence terms. Leave one sentence in
  place and move the details to a separate surface: **了解更多**, **技术详情** or
  a documentation page. [5.2.5]
- Reassure once, and only where the reader would worry: a destructive action, a
  change to their worlds. Say what they will find ("世界和设置保持不变"), not
  what the launcher avoids doing.
- Nothing misleading: no reassurance that is not always true, no risk left
  unsaid.

## 4. Put it in order [5.2.2]

- The first sentence carries the most important message: the result, the
  decision or the risk.
- What every reader needs comes before what only some readers need (third-party
  accounts, one platform, one loader). Often the second kind belongs in its own
  sentence or on another surface.
- When an action can lose data, the warning comes before the instruction.
- Steps go in the order they happen.
- The body and the keys tell the same story. If the body says to do
  something first ("装好后再试"), no key may read as if it goes on at once.
- Start from what the reader knows: name the game's thing (世界, 存档) before
  the launcher's thing (快照, 预设).

## 5. Write the sentences [5.3]

- One fact per sentence. In body and help text, do not chain facts with "；" or
  put a clause in "（）"; make separate sentences or cut. Brackets may hold a
  unit or a short gloss of a value ("（游戏目录）").
- Use the player's words. Use a launcher term (快照, 预设, 加载器) only for a
  thing the launcher shows by that name, and use the same word each time (the
  term list in `.agents/skills/lumilio-i18n/translate-prompt.md`).
- A name the reader must judge, such as a service that receives data or a
  component that gets installed, gets a short gloss the first time: "MCIM
  镜像", "皮肤插件 authlib-injector".
- The subject is the player or what they see, not the launcher's inner parts:
  "要重新登录", not "会从系统凭据库删除它的登录信息".
- Short, not cryptic: never drop the object or the consequence to save
  characters.
- Lists of values (variables, file types, keys) go in a list, a table or the
  documentation, not in a sentence.
- The tone is §8's: calm, direct, never blaming. [5.3.7]

## 6. Fit the space

A message fits the place where it appears. A ⓘ hint (`kit::info`, and every
settings-row help through `kit::value_row`) and a key's tooltip have no
maximum width: they never wrap, so a long hint runs past the window. The
English is 1.5–2× as wide as the Chinese (§8), so the Chinese limit leaves
room for it.

| Where | Limit |
| --- | --- |
| ⓘ hint, a key's tooltip | One sentence, 20 characters |
| Note or help under a field | One sentence, 20 characters |
| Empty state | One sentence, 20 characters |
| Confirmation body, dialog intro | Two sentences, 35 characters |
| Failure caption or toast | One sentence and the action, 35 characters |
| Screen-reader label | 10 characters |

Count Chinese characters; a Latin letter, digit or space counts as half, and an
argument as one. A command or address the reader must copy is not counted.

The first three rows are hints, and their ids end in `-help` or `-note`. The
test `hints_fit_one_line` holds every hint to one sentence of 20 characters
and its English to twice that width. `crates/lumilio-ui/src/i18n/long-hints.txt`
lists the hints not revised yet; it may only shrink.

When the facts the reader needs do not fit, the surface is wrong, not the
limit: give each key its own tooltip, or move the details to the documentation.
Fitting the limit does not prove a message works; readers decide that (§7).

## 7. Check with readers [5.4]

Only readers show that a message works.

1. **While writing: a cold read.** Someone who has not seen the code or the old
   wording reads only what is on the screen (title, body, keys) and answers:
   what is this, what happens if I choose each key, what do I lose and what is
   kept. A wrong or guessed answer about the main action's consequence means
   rewrite; the fix is usually a better word, not another sentence. An agent
   without the change's context can do this read
   (`.agents/skills/lumilio-i18n/plain-review-prompt.md`).
2. **Before merge.** The Project Maintainer reviews screenshots of each changed
   surface in Chinese, and in English at the 720 px window (§8).
3. **After release.** A report that players misread a message is a catalog
   bug.

## 8. English

The same content in the same order. Write the English from the revised
Chinese meaning, not from the old English (§8, `translate-prompt.md`). The
English may split or join sentences differently; it never adds or drops a
fact.

## Examples

**Removing the current, signed-in account** (confirmation body)

- Before: 会忘记这个身份并从系统凭据库删除它的登录信息（第三方账户还会通知服务器作废令牌），不会删除任何游戏或存档。它是当前账户，移除后会改用剩下的第一个。
- After: 重新登录就能再用，游戏和存档不受影响。
- Why: the reader is deciding (§2), so the consequence they act on comes first.
  The credential store and the token are how it works (§3). Which account
  becomes current is not needed to decide, and the account chip shows it
  afterwards; the old "改用剩下的第一个" was also false for the only account.

**Repairing game files** (ⓘ hint)

- Before: 逐个核对游戏文件，缺的或损坏的会重新下载；世界、Mod 和设置不会被改动。
- After: 补齐缺失或损坏的文件，世界和 Mod 不变。
- Why: a ⓘ hint has one line (§6). The result comes first (§4), and the
  reassurance names what the player will look for (§3).
