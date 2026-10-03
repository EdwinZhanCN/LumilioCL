---
name: lumilio-ia-paths
description: Use when adding, changing or removing anything a person can do on a
  LumilioCL page — declare the user path with a `// ia[page]:` comment at its
  code, regenerate `docs/ia/paths/`, and keep the not-built list honest.
---

# Declare User Paths In Code

User paths (what a person can click, drop, type or choose, and what happens) are
not written in documents. The comment at the code that implements the path is the
only source; `lumilio-docgen` turns the comments into `docs/ia/paths/<page>.md`
(ADR 0019). A path is in the generated table if and only if its comment exists,
so every row means "built".

## The comment

```rust
// ia[library]: 排序 / 按加载器筛选 | L4 两个下拉 | 记在偏好设置里，下次打开还是这样 | H-NAV-04
```

Four fields separated by `|`, plus an optional fifth:

| Field | Write |
| --- | --- |
| 操作 | What the person does, in the words of the screen. Unique within the page |
| 层 / 组件 | Where it is: the layer (L1–L6, `docs/design-language.md` §7) and the control |
| 结果与反馈 | What happens, including the feedback and the failure behaviour worth knowing |
| 编号 | Flow ids from `docs/workflows/` (`H-…`, `L-…`; `H-CONTENT-06/07` means two), or `—` |
| 备注 (optional) | A caveat, an ADR, or a limit ("运行中禁用") |

No `|` inside a field. Page keys are the `PAGES` table in
`crates/lumilio-docgen/src/lib.rs`: `navigation`, `home`, `library`, `discover`,
`activity`, `accounts`, `settings`, `instance`, `instance.overview`,
`instance.content`, `instance.worlds`, `instance.history`,
`instance.diagnostics`, `instance.settings`. A new page is a new row there.

## Where it goes

Put it directly above the statement that creates the entry point — the `Key`,
`MenuEntry`, input, drop handler or row — so the comment moves or dies with the
code. One comment per entry point; a menu with five entries gets five comments.
Never put one on a handler far from the control: it will outlive the control.

## Procedure

1. Change the UI code and add, edit or delete its `// ia[...]` comments in the same change.
2. Run `cargo run -p lumilio-docgen -- ia`. It validates the comments (known page,
   known flow ids, no repeated action) and rewrites `docs/ia/paths/`.
3. Commit the regenerated files with the change. `cargo test` fails with
   `docs/ia/paths is stale` if you forgot.
4. If the change builds something listed in `docs/ia/README.md` under "没做的 /
   范围外", delete that row. If it deliberately leaves something out, add a row
   with the reason or the ADR it waits for.
5. A path outside `ARCH.md` needs an ADR before it gets a comment.

## Failure modes

- **A mistyped flow id** fails the scan; look it up in `docs/workflows/`. If the id
  is real but new, define it there first.
- **A comment that describes the intent, not the code** passes every check. When
  you change behaviour, reread the comment; the checks cannot.
- **Moving a file** changes only the 实现 column; regenerate and commit.
- **Editing `docs/ia/paths/` by hand** is overwritten on the next run.
