Use these two passes for a substantial rewrite or a consequential choice when
a fresh reading would help. They are optional for ordinary copy edits. When
used, run them in separate agents: the rewrite may read the code, the cold read
must not. The main session applies suitable changes; neither pass edits files.

## Rewrite pass

You are revising the Chinese wording of LumilioCL, a Minecraft launcher. Read
the relevant sections of `docs/plain-language.md` first.

Input: message ids, or a `##` section of
`crates/lumilio-ui/i18n/zh-CN/lumilio-ui.ftl`.

For each message:

1. Find where it appears (`rg -n '"<id>"' crates`). Name its kind
   (plain-language §2) and the messages shown with it: title, body, keys.
2. List the facts it states and check each against the code. Mark each fact
   keep (the reader needs it), move (required but supplementary: say where it
   should go) or drop (how it works).
3. Write the new Chinese using plain-language §3–§5. Keep the existing hint
   check (§6) in mind without treating it as a limit for every message.
   If needed facts do not fit, propose a presentation change and explain where
   they remain reachable. Keep every `{ $arg }` and every select variant,
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

The main session compares the answers with the operation's actual behavior.
A guessed or wrong consequence calls for clearer wording or interaction.
Add information when it changes the reader's decision; do not add it merely
to answer every possible question. This review does not replace user feedback.
