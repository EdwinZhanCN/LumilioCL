# 0035 — Select 原生支持多选

- Status: accepted
- Date: 2026-10-07

## Context

日志来源和级别筛选使用页内操作菜单，与其他页面的 Select 不一致。维护者要求直接扩展 Select，并把两个下拉放在搜索右侧，与操作按钮同行。

## Decision

维护锁定的 gpui-component 0.7.0 源码快照，通过 Cargo `[patch.crates-io]` 让 UI 和 GPUI Kit 使用同一实现。沿用 Select 的主题、列表、键盘和焦点机制；新增 `SelectState::new_multiple`、完整选择读写和 `SelectEvent::Change`。

多选时点击或 Enter 独立切换当前项，菜单保持打开；Escape、失焦或点击外部关闭并保留已提交的选择。单选保留原来的 Confirm 事件和选择后关闭行为。触发器与辅助技术展示完整多选值，超长触发器文字沿用 Select 的截断。

日志工具栏顺序为搜索、来源单选、级别多选、分析、复制、导出。窄窗口横向滚动，保持单行。

## Consequences

页面不再自己构造多选菜单；升级 gpui-component 时需保留该扩展并运行 UI crate 的 Select 交互测试。许可证、基线和导入范围记录在 ATTRIBUTIONS.md 与 forks/README.md。

## Shipped

Select 多选、日志页接线与单行工具栏、生成的 IA；交互测试覆盖单选、多选、键盘、清空和关闭后的选择保留，日志页测试覆盖事件接线和同行布局。维护者明确跳过本次 `just check`；实机视觉验收仍由维护者接手。
