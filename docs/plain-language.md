# LumilioCL Plain Language

Writing guidance for the launcher's interface, in Chinese and English. Read it
with the screen and task in mind. These are defaults, not a sentence template
or a reason to remove information the feature needs. Wording and interaction
can change together; a local choice needs no separate design approval.

The direction follows [ISO 24495-1:2023](https://www.iso.org/standard/78907.html):
help readers get what they need, find it, understand it and use it. The advice
below is the project's adaptation, not a reproduction or compliance claim.

## 1. Readers

Write for a player trying to play, manage games or solve a problem. Use familiar
game vocabulary; explain launcher-specific concepts when they affect a choice.
An expert tool may need technical terms and denser information than Home.

## 2. What each kind of message is for

| Kind | Useful starting point |
| --- | --- |
| Action | Name the result: 导出材料清单 / Export material list. |
| Title | Identify the object, task or decision. |
| Confirmation | Explain scope, consequence and recovery where they matter. |
| Setting help | Explain what changes for the player. |
| Failure | Say what failed and give a useful next step, if one exists. |
| Empty state | Explain what belongs here or why nothing matches. |
| Placeholder | Give an example or format; keep a real field label. |
| Accessible name | Identify the control; expose state through its semantics. |

Use only the information the surrounding screen does not already supply.

## 3. Choose the content

Keep facts that affect the decision or completion of the task: the target,
data loss, required sign-in, restart, permissions or a service receiving data.
Supplementary implementation detail can live in technical details or help.
Do not hide a necessary instruction behind a tooltip or an unreachable guide.
Reassurance must be true for every case where it is shown.

## 4. Put it in order

Lead with the result, decision or problem. Put consequences before committing
an irreversible action and steps in execution order. Read the title, body and
buttons together: they should describe the same action and scope. Split or
group information according to the task, not a fixed sentence count.

## 5. Write the sentences

Prefer direct, calm language, specific verbs and consistent terms. Short is
useful when it remains clear. Avoid blame, ritual confirmations and unnecessary
mechanism explanations; technical views may need precise diagnostic language.
Use sentence case in English and preserve proper names. Use `…` when a command
requires another choice before it acts. Follow the platform's own names, such
as “Show in Finder”. Playful world captions should not obscure actionable text.

The shared Chinese–English terms are in the
[translation reference](../.agents/skills/lumilio-i18n/translate-prompt.md).
Examples illustrate intent; they are not wording to copy into every screen.

## 6. Fit the space

Judge copy in its actual control, with real variable values, both languages
and supported window sizes and text scales. A longer label can be clearer;
wrapping, a wider surface or visible help may be better than deleting meaning.
There is no universal length limit for dialogs, empty states or accessible names.

One existing implementation guard still applies: `hints_fit_one_line` in
`crates/lumilio-ui/src/i18n/tests.rs` checks ids ending in `-help` or `-note`.
It permits at most 20 weighted Chinese characters and one sentence, and an
English weighted length of 40. Han characters count as one, Latin letters,
digits and spaces as half, and a placeable as one; this is a catalog estimate,
not rendered width. `long-hints.txt` tracks existing exceptions and only shrinks.

Keep that check passing while it exists. If a feature needs richer help, change
the surface and its validation together rather than silently dropping facts or
renaming an id to evade the check. Other word counts are editorial judgments.

## 7. Check with readers

Review the changed words in context: can a reader tell what happens, to which
object, and what can be recovered? A cold read can expose ambiguity in a
consequential dialog or unfamiliar flow; it need not be a separate ceremony
for every label. An agent's cold read is an aid, not evidence from actual users.
For layout-sensitive changes, inspect the rendered Chinese and English with
representative long content. Reports of misunderstanding are feedback to improve
the words or interaction, not automatically a reason to add another rule.

## 8. English

Preserve meaning, consequences and scope; write natural English rather than
mirroring Chinese sentence structure. Use the same terms across related
controls, translate whole messages, and handle counted nouns with plurals.
Length expansion varies: inspect the result instead of assuming a fixed ratio.

For example, “读取失败” can become “读不到版本列表，请重试” when the object and
recovery are otherwise unclear. In a dialog already titled “删除游戏？”, the
body should add the actual deletion scope or recovery information, not repeat
the question. Verify those facts against the operation before promising them.
