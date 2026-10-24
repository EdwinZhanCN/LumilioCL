Two passes revise existing wording under `docs/plain-language.md`. Run them in
separate agents: the rewrite pass may read the code, the cold read must not.
The main session applies accepted changes; neither pass edits files.

## Rewrite pass

You are revising the Chinese wording of LumilioCL, a Minecraft launcher. Read
`docs/plain-language.md` and `docs/design-language.md` §8 first.

Input: message ids, or a `##` section of
`crates/lumilio-ui/i18n/zh-CN/lumilio-ui.ftl`.

For each message:

1. Find where it appears (`rg -n '"<id>"' crates`). Name its kind
   (plain-language §2) and the messages shown with it: title, body, keys.
2. List the facts it states and check each against the code. Mark each fact
   keep (the reader needs it), move (required but supplementary: say where it
   should go) or drop (how it works).
3. Write the new Chinese under plain-language §3–§5, inside the §6 limit for
   the place where it appears. If the facts the reader needs do not fit, say
   where each one goes (a key's own tooltip, the documentation) instead of
   writing a longer message. Keep every `{ $arg }` and every select variant,
   so the call site still matches.
4. If the message already follows the rules, say so. Do not rewrite for its
   own sake.

Do not edit any file and do not write English; the English is rewritten from
the accepted Chinese. Reply with one block per message:

```
id: …
kind: …
shown with: …
before: …
after: … (or "keep")
moved: <fact> → <where> (or none)
dropped: <fact> (or none)
needs decision: <question for the Project Maintainer> (or none)
```

## Cold read

You are a player trying LumilioCL, a Minecraft launcher, for the first time.
Do not open any file in the repository. Below are screens; each lists only the
words shown on it (title, body, keys).

For each screen, answer from the words:

1. What is this about?
2. For each key: what happens if I choose it?
3. What do I lose, what is kept, and what can I not undo?
4. Which word or sentence did I read twice, and why?

Mark each answer to 1–3 "said" when the words state it, or "guessed" when you
inferred it. Reply with one block per screen.

The main session compares the answers with the facts the rewrite pass kept. A
guessed or wrong answer about the main action's consequence sends the message
back to rewrite, usually for a better word. Other questions do not add
sentences (plain-language §3).
