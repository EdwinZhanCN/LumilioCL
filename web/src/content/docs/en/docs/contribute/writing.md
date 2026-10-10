---
title: Write documentation
description: Use ASD-STE100 writing rules and consistent project terms.
---

Use ASD-STE100 Simplified Technical English for published documentation.
Read the [official specification](https://www.asd-ste100.org/) for the dictionary and complete rules.

## Write instructions

1. Use the command form for an instruction.
2. Write one action in each sentence.
3. Keep each procedural sentence at 20 words or fewer.
4. Put the condition before the action.
5. Use the active voice.
6. Use the same term for the same item.
7. Keep each paragraph at six sentences or fewer.

Keep descriptive sentences at 25 words or fewer.
Use approved dictionary words with their approved meanings.
Use necessary software names as technical terms.
Do not remove necessary information to shorten a sentence.

## Technical terms

| Term | Meaning |
| --- | --- |
| Project Maintainer | A person or Coding Agent that does project work |
| Coding Agent | Software that examines files, changes code, and runs commands |
| repository | The Git project and its tracked files |
| crate | A Rust package |
| JSON plan | A retained execution record in `.agents/plans/` |
| acceptance condition | A required result that you can check |
| handoff | The delivery of a change with verification results |
| user path | A declared action and its feedback in the launcher |
| regression test | A test that detects a previously observed defect |
| WASM plugin | A proposed plugin compiled to WebAssembly |
| API | A defined interface between software components |
| dependency | A software package that another package needs |
| toolchain | The compiler and related tools for a specified version |
| UI | The controls and displays through which a person uses the application |
| CI | Automated project checks that run in GitHub Actions |
| checksum | A calculated value used to check file contents |
| pull request | A proposed Git change submitted for review |
| fork | A separate copy of an upstream project |
| branch | A named line of work in Git |
| skill | Repository instructions for a specific kind of project work |
| manifest | A file that describes a package and its requirements |

Keep commands, API names, filenames, and UI labels exact.
Use code formatting for these identifiers.
Keep official product names unchanged.

### Technical verbs

These software verbs have the following meanings:

| Verb | Meaning |
| --- | --- |
| build | Generate a software application from source files |
| compile | Translate source code into executable code |
| run | Start a program, command, or check |
| install | Put software in its required location |
| clone | Make a local copy of a Git repository |
| commit | Record a set of changes in Git |
| push | Transfer Git commits to a remote repository |
| merge | Combine changes from Git branches |
| block | Prevent calling code from continuing while an operation waits |

## Add a page

1. Add a Chinese page under `web/src/content/docs/docs/`.
2. Add its English page under `web/src/content/docs/en/docs/`.
3. Add `title` and `description` fields to the frontmatter.
4. Add the page to the sidebar in `web/astro.config.mjs`.
5. Check each technical claim against the current source.
6. Run `just web` from the repository root.
7. Examine the page on desktop and mobile.

Use the same relative file path for both languages.
Component text uses `web/src/i18n/docs.ts`.
Starlight interface translations use `web/src/content/i18n/`.
Sidebar labels use Starlight's `translations` field.

Use `features/` for feature procedures.
Use `wasm/` for the future plugin contract and development procedures.
State the implementation status before you describe an unfinished capability.
Forwarded release notes keep their source language.
Do not translate them automatically.

## Review the language

Check the sentence lengths and technical terms.
Check each instruction for one clear action.
Check dictionary meanings and parts of speech.
Automated length checks do not certify ASD-STE100 compliance.
The Project Maintainer must also review the language.
