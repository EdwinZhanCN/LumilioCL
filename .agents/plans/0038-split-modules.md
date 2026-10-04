# 0038 — 三个 crate 按职责拆分模块

- Status: done
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
- [x] T2: `lumilio-ui`：`pages/live`、`shell`、`instance_detail`（含 content/panels/settings/diagnostics）已拆；`live`、`home`、`pages/settings` 也已拆。
- [x] T3: `lumilio-core`：`service.rs`（约 9000 行）已拆成 `service/`（含 `tests/`）。`install.rs`（1448）、`launcher.rs`（1315）、`transfer.rs`（1274）、`discover.rs`（1167）以及 `ui/kit.rs` 也已拆。
- [x] T4: 文件规模和测试放置的约定写进 AGENTS.md「Tech stack & structure」。

## Validation

每个任务单独提交，提交前跑四项检查；拆分前后 `cargo test` 的测试数量一致；`cargo run -p lumilio-docgen -- ia` 只改“实现”列。

## Outcome

（逐步追加）

- 拆分用一个临时脚本（不进仓库）完成，按行号指定每个顶层项/impl 方法归属，自动生成各子模块的 `use`，再用编译器诊断清理无用导入。拆分前后测试数量逐个文件核对一致，公共路径靠 `pub use` 保持不变。
- 过程中踩过的坑：只在“被点名”时才导入会漏掉 trait 的方法调用和格式化字符串里的 `{NAME}`；编译失败时的“未使用导入”不可信，清理要等构建通过；跨层移动要把 `super::` 路径和 `pub(super)` 同步加深一层。
- 最后一轮把剩下的 install / transfer / launcher / discover / kit 五个文件一起拆完，只跑了一次完整检查。
