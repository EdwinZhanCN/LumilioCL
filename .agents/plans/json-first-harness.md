# JSON-first Agent Harness 与公开 Roadmap

- Status: proposed

## Goal

把 `.agents/plans/` 从临时 Markdown 计划改为可验证、可保留、可投影的 JSON 生命周期记录。JSON 是唯一可编辑数据源；Markdown 与网站 Roadmap 是确定性生成物。简化未来新建 decisions/postmortems 的常规流程，但保留历史 ADR、事故记录与现有代码引用。普通小修复继续不强制写 Plan。

## Scope

- In：版本化 Plan Schema、JSON 创建/更新流程、任务与决策及验收结果、Markdown 生成、公开 Roadmap 投影、当前进行中/拟议计划的渐进迁移、Agent Skills/README/CI、现有 `web/` 集成。
- Out：修改 LumilioCL 产品架构、插件 WASM 运行时、引入数据库/托管服务、新的 issue tracker、重写全部历史 ADR、对外承诺发布日期。
- 当前仓库重要事实：`crates/lumilio-docgen` 已负责从代码生成 IA；`just docs` 目前只检查 docgen 与 rustfmt；`ci.yml` 对 `.agents/**` 和大部分 Markdown 排除了触发；`web/` 已有 Astro + React Roadmap，数据手写在 `web/src/i18n/zh-cn.ts`，`Sections.astro` 消费 `t.roadmap.items`，Logo 三个灰格仍由 `web/src/components/hero/plate.ts` 的静态布局承载。
- 此计划的提出相当于替代 ADR 0021 中“完成后生成 ADR 并删除 Plan”的生命周期政策；ADR 0021 本身保留为历史记录，不回写改写。

## Decisions / Design Constraints

1. `.agents/plans/<slug>.json` 为主数据源，`.agents/schemas/plan.schema.json` 为 v1 公开契约。ID 不依赖文件名或序号，创建后稳定；给每个任务分配稳定 ID，避免 Git diff 里频繁重排。
2. Status：`proposed | in_progress | blocked | completed | cancelled`。`completed` 表示工作及验收完成，**不等于 released/shipped**。公开版本与实际发布事实以 Release/发布代码为准。
3. 最小必填：`schemaVersion`、`id`、`title`、`summary`、`status`、`scope`、`tasks`、`validation`。可选：`decisions`（含 rationale/consequences）、`references`、`dependencies`、`outcome`、`lessonsLearned`、`roadmap`。
4. 独立的 `visibility` 与 `roadmap.enabled`：不公开内部字段。公开投影必须按字段白名单输出，不能直接复制整个 Plan。Roadmap 允许版本目标但不能推断发布日期。
5. 新功能普通决策和复盘写回 Plan；旧 `.agents/decisions/`、`.agents/postmortems/` 变为只读历史资料，保留已有 ADR 引用。跨多个 Plan 的长期架构契约通过代码/API/设计文档及稳定引用表达，不能藏进一次性任务。
6. 大型旧计划（如 `world-explorer.md`）不能机械压缩丢失信息：JSON 收录当前目标、状态、验证、剩余任务；旧 Markdown 冻结到 legacy/archive 路径或保留明确 Git permalink。冻结内容不得继续作为第二份活跃状态。
7. 允许贡献者仅用 Issue/PR 修复小 Bug。Plan 用于跨会话、多任务或架构变更，不能成为所有贡献的前置门槛。
8. 不改变已生成的 `docs/ia/paths/` 语义：它仍只描述**已实现**功能；Roadmap 只描述计划，不许据此生成“已实现”用户路径。

## Tasks

- [ ] P1 — Schema + reference examples：定义 v1 Schema、合法/非法 fixture、字段语义；覆盖重复 ID、未知 status、缺少验收条件、无效关联。根据实际任务决定是显式 JSON Schema 校验库还是 Rust 强类型验证，但必须由 CLI 和 CI 执行校验，不只是提供 schema 文件。
- [ ] P2 — Generator：优先扩展 `crates/lumilio-docgen` 或拆出独立清晰模块，实现 `plans validate`、`plans generate`、`plans check`；确定性从 JSON 生成 `docs/plans/*.md` 和 `web/src/data/roadmap.generated.json`，固定顺序，UTF-8，跳过私有字段；输出头部标记不可手改。必要时给 `justfile` 增加 `plans`/`plans-check`，并让 `just docs` 覆盖检查。
- [ ] P3 — Site integration：`web/src/components/Sections.astro` 从生成的 JSON 消费公开 Roadmap；本地化文案可以保留在站点目录，但与 Plan 同一事实不能手写双份。Logo 三个灰格是固定位置的 curated teaser，引用稳定 Plan/roadmap ID 而不是直接映射任意数量的计划。现有 `0.1.1 / 0.2.0 / 0.3.0` 文案须逐条与计划关联后再切换；发布版本仍来自真实 Release。
- [ ] P4 — Migration：逐个迁移真正 active/proposed 的计划（至少检查 `launcher-site`、`world-explorer`、`wasm-plugin-registry` 及其引用）；把未排期点子保留在 `backlog.md` 或显式候选计划；已完成 ADR/Postmortem 不进行无差别改写。冻结旧入口并保证不出现并行编辑的活跃副本。
- [ ] P5 — Atomic harness switch：P1–P4 通过后，在同一变更中修改 `AGENTS.md`、`.agents/README.md`、`.agents/plans/README.md`、`.agents/skills/lumilio-exec-plan/SKILL.md` 以及需要调整的 skills/引用。废除“Plan → ADR → 删除 Plan”，改为创建 → 跟踪 → 验证 → outcome → 保留 JSON。保留 ADR 0021 原文及旧编号引用索引。
- [ ] P6 — CI verification：`.github/workflows/ci.yml` 继续检查 Rust；另加对 `.agents/plans/**/*.json`、Schema、生成器、`docs/plans/**`、Roadmap 生成文件及相关工具改动触发的验证工作流，确保 docs-only PR 也有自动检查。测试生成可重复、过时 Markdown 检测、私有信息不会泄漏、无效 Schema/状态会阻止合并，并对 `just web` 做回归验证。

## Validation

- 新建一个 proposed JSON Plan，生成可读的 GitHub Markdown，并能更新任务状态、添加验收记录和 outcome；标记 completed 后源 JSON 不被删除。
- 相同输入运行两次，生成字节一致；手改生成文件或源字段无效时 `plans check` 失败。
- `visibility: internal`、`roadmap.enabled: false` 的项目不会进入 `web/src/data/roadmap.generated.json`；任何内部决策细节都不能通过嵌套字段意外泄漏。
- 网站 Roadmap 和 Logo 固定灰格都能从有效计划读到各自需要的数据；公开计划不能被误标为“已发布”。
- `just docs`、`just web` 与 `just check` 在适当阶段验证通过；独立 Plan CI 在只改 `.agents/plans/**` 的 PR 也执行。
- 历史 ADR、Postmortem、`plan-history.md` 以及代码中对旧 `ADR NNNN` / `plan NNNN` 的引用仍能访问。
- 新 Agent 只靠 `AGENTS.md` + plan skill 能执行生命周期；小型 PR 不会被迫创建 JSON Plan。

## Handoff

不要在生成器和 Schema 准备好之前先改 Agent 的日常工作指令；要原子切换，否则旧技能与新格式会冲突。先按 P1–P6 分阶段实现，逐阶段执行验证并记录当前阻塞。完成后报告迁移清单、变更命令、测试与公开信息边界。
