# 0036 — 默认进入真实界面，移除示例预览骨架

- Status: done
- Phase: Phase 8 — ARCH 页面接入和视觉验收
- Author: agent

## Goal

`cargo run -p lumilio-app` 直接打开真实启动器（打开数据目录、接入 core）。示例数据预览（计划 0004）和启动时刻脚本预览（ADR 0005 的 `LUMILIO_PREVIEW`）不再存在；打不开数据目录时报错退出，不回退到示例界面。

## Scope

- In scope：删除 `lumilio-app/src/preview.rs` 与 `LUMILIO_PREVIEW`；删除 `lumilio-ui` 的 `sample` 数据、`with_preview`、`Location::Sample`、仅服务预览的 `pages::{activity,discover,home,library,instance}`；把真实代码仍在用的类型（如 `Loader`）移出 `sample`；更新 README、roadmap、IA/design-language/ADR 0005 里提到预览的地方。
- Out of scope：真实界面的任何行为改动；`LUMILIO_PAGE`、`LUMILIO_INSTANCE`、`LUMILIO_REVIEW_*` 等评审钩子（真实界面也用）保留。

## Tasks

- [x] T1: app：`main.rs` 始终打开 backend；删除 `preview.rs`。
- [x] T2: ui：删除 shell 里的预览分支与 `Location::Sample`；`Controls` 等仅预览用的状态一并删除。
- [x] T3: ui：删除预览页面模块与 `sample.rs`，保留的类型迁到所属模块。
- [x] T4: 文档：README / roadmap / IA / design-language / ADR 0005 / history。
- [x] T5: 四项检查全绿；`LUMILIO_HOME=<临时目录> cargo run -p lumilio-app` 实机能打开。

## Validation

`cargo build`、`cargo test`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check`；被删页面里带的测试若在测真实行为，要迁移而不是丢弃。

## Risks / open questions

- `ViewState` / `ViewIntent` / `kit` 里有真实界面也用的部分，删除要以编译器为准，逐步收缩。
- ADR 0005（启动会话契约）仍然成立，只是不再有脚本化预览驱动它；ADR 本身不改写。

## Outcome

- app：`main.rs` 始终打开 backend，失败就退出；`preview.rs` 删除。
- ui：删除 `sample.rs`、预览页面（activity/discover/home/library/instance）、`Controls`/`Ctx`、`with_preview`、`Location::Sample`；`Loader` 迁到 `cover`；`ViewIntent` 只留 `LibraryTab`/`ActivityTab`/`Choose`；`show()` 不再接受实例参数。
- 测试：删除“每个 ARCH 页面可达”的预览测试（它测的是示例页）；Home 滚动测试保留，去掉发现页和实例页两段（它们依赖示例页面）。
