# 0038 — 三个 crate 按职责拆分模块

- Status: in_progress
- Phase: Phase 8 — ARCH 页面接入和视觉验收
- Author: agent

## Goal

没有一个源文件把几件不相干的事混在一起：每个模块一件事，测试放在自己的文件里，文件名说明里面是什么。拆分不改行为：每一步前后 `cargo test` 结果相同，`cargo run -p lumilio-docgen -- ia` 只有“实现”列的文件名变化。

## 约定

- 超过约 800 行的文件按职责拆成目录模块：`name/mod.rs` 只放对外接口、共享类型和装配，各职责一个文件。
- 单元测试放在同目录的 `tests.rs`（或 `tests/<主题>.rs`），用 `#[cfg(test)] mod tests;`，不再和实现挤在一个文件里。
- 子模块之间用明确的 `use`，不用 `use super::*`（测试模块除外）；跨模块的项用 `pub(super)`，只有真正的对外接口才 `pub`。
- 不改公共 API 的路径（`lumilio_core::X`、`lumilio_ui::X` 保持可用，必要时在 `mod.rs` 里 `pub use`）。

## Scope

- In scope：`lumilio-app`（`live.rs`）、`lumilio-ui`（`shell.rs`、`instance_*.rs`、`live.rs`、`pages/live.rs`、`pages/settings.rs`、`kit.rs`、`home.rs`、`new_game.rs` 等）、`lumilio-core`（`service.rs` 及其他超大文件）。
- Out of scope：任何行为改动、重命名公共类型、依赖变化。

## Tasks

- [x] T1: `lumilio-app/src/live.rs` → `live/`（mod、dispatch、jobs、library、accounts、new_game、instance、instance_write、settings、launch、discover、tests；3375 行 → 最大 743 行）。
- [ ] T2: `lumilio-ui`：`pages/live.rs`、`shell.rs`、`instance_*.rs`、`live.rs`，以及其余超过约 800 行的文件。
- [ ] T3: `lumilio-core`：`service.rs`（约 9000 行）按领域拆成 `service/`，其余超大文件视情况。
- [ ] T4: 在 `lumilio-select-checks` 或 AGENTS.md 里写下文件规模和测试放置的约定。

## Validation

每个任务单独提交，提交前跑四项检查；拆分前后 `cargo test` 的测试数量一致；`cargo run -p lumilio-docgen -- ia` 只改“实现”列。

## Outcome

（逐步追加）
