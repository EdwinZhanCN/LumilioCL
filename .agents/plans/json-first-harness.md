# JSON-first Agent Harness：一次性替换

- Status: proposed

## Goal

一次 PR 完成 LumilioCL Agent Harness 的 **直接切换**：`.agents/plans/*.json` 成为唯一的 Plan / Decision / Outcome / Incident 工程记录。JSON → Markdown 和公开 Roadmap 均为确定性生成物。网站同步摆脱硬编码路线图，并增加独立的 `/roadmap` 与 `/changelog` 页面。**无过渡模式、无双轨写入、无 legacy 只读目录、无分批合并。** 保留有价值的工程知识，但不保留旧系统的持续维护入口。普通小改动不强制 Plan。

本计划是实施清单，尚非新格式的范例。完成重构时它本身也必须转换成 JSON，旧 Markdown 删除。

## Scope / Current Facts

- In：版本化 Schema、校验器、生成器、Markdown 与公开 Roadmap、`primary` 展示规则、`/roadmap` 和 `/changelog` 页面、现存文档数据的一次性迁移、`AGENTS.md` / Skills / README 更新、CI、历史引用修复。
- Out：修改桌面应用实际功能、实现社区 WASM 插件、重造 issue tracker、引入数据库或托管后端、承诺具体发版日期。
- 既有 `crates/lumilio-docgen` 从代码生成 `docs/ia/paths/`；这套 IA 仍只陈述已实现功能，不应混入 Roadmap。
- 现有 `web/` 是 Astro + React + Worker；Roadmap 的手写条目位于 `web/src/i18n/zh-cn.ts`、`web/src/components/Sections.astro`，Logo 的三个灰格由 `web/src/components/hero/plate.ts` 静态定义，并在 `Faceplate.tsx`、`Explorer.tsx` 中按静态 key 消费。保留九宫格布局与三个灰格的位置，但移除固定计划 ID / 内容。
- 现有 `web/src/data/release.ts` 在 Astro 构建时获取 GitHub **最新正式发布的 Release**（`releases/latest`），并从 release assets 构造下载地址。新增 Changelog 应基于 GitHub **已发布 Releases 列表**而非 Plan status；可以复用读取方式，但需包含有意义的 prerelease 版本及发布说明，并考虑构建时 API 失败的降级状态。现有 `web.yml` 已在 `release: published` 时重新构建。
- 当前 `ci.yml` 排除了 `.agents/**` 和大多数 Markdown；`just docs` 不验证 Plan 文件。
- ADR 0021 曾要求“完成 Plan → 写 ADR → 删除 Plan”；这是要由本次变更正式替代的旧政策，**不要再沿用或留第二种可写流程**。

## Final Tree (target, not incremental)

```text
.agents/
├── README.md
├── plans/
│   ├── README.md
│   ├── <stable-slug>.json
│   └── ...
├── schemas/
│   └── plan.schema.json
└── skills/
    └── ...
```

- **移除** `.agents/decisions/` 和 `.agents/postmortems/`，不保留 `legacy/`、`archive/` 等新形式的旧目录。
- `backlog.md` 不再是另一份权威计划状态；未立项点子可以放 GitHub Issues，或建立 `proposed` JSON Plan。
- 关键历史事实需要迁入关联 Plan 的 `decisions` / `outcome` / `lessonsLearned`，或进入代码/API/设计规范；纯历史细节留在 Git 记录中。不得遗漏仍被其他文件引用的架构约束。
- 不必机械逐字保存所有过时文档，但删除前必须逐一审计引用，不能留下断链。旧编号引用（`ADR NNNN`、`plan NNNN`、postmortem 文件名）重指向新 Plan 或相关规范；仅需追溯原始文本时，使用固定 Git commit permalink，不恢复旧的可写目录。

## Data Contract

- Source of truth：`.agents/plans/<slug>.json`。
- 必填：`schemaVersion`、`id`、`title`、`summary`、`status`、`scope`、`tasks`、`validation`。
- 可选：`priority`、`targetVersion`（计划发布版本）、`primary`（布尔值，默认 false）、`dependencies`、`references`、`decisions`（含 context / rationale / consequences）、`outcome`、`lessonsLearned`、`releasedIn`（实际发布版本，必须能匹配真实 Release）。
- Plan 状态：`proposed | in_progress | blocked | completed | cancelled`。已完成不等于已随 Release 发布。所有 task ID 与 Plan ID 稳定且唯一；验证条件与执行证据明确。
- Plan 应能同时描述新功能、架构性工作以及事故修复，不需要独立 ADR 和 Postmortem 数据模型。
- **唯一网站展示开关是 `primary`**：仅 `primary: true` 的 Plan 出现在公开 Roadmap 和首页展示；不再引入 `visibility`、`roadmap.enabled` 或 major/minor/patch 分类开关。`primary: true` 必须声明可解析的 `targetVersion`，计划标题与摘要需适合公开。默认 false 的 Plan 不公开。公开数据仍由字段白名单投影，不得直接序列化内部记录。
- 所有公开 Plan 按 `targetVersion` 的 SemVer 升序排序（不区分版本级别、无 major/minor/patch 分组），版本相同时按 `id` 排序。取 `primary: true` 中排序后的 **前 3 个**用于首页九宫格灰格和首页 Roadmap 摘要；`/roadmap` 展示**全部** primary Plan。不得依赖文件系统顺序或手写 ID 白名单。
- 当 primary Plan 不足 3 个时，不得注入假计划或回退硬编码内容。保留九宫格视觉布局，用非计划的中性空态或跳往 `/roadmap` 的说明占位，且保持键盘/无障碍导航完整；是否可点击由真实交互决定。
- `targetVersion` 是计划目标，不表示承诺发布日期；`releasedIn`（如使用）是事后验证的事实，**不得**因为 Plan status=completed 就自动填入。
- Changelog 的权威来源是 GitHub 已发布的 Releases（tag、published_at、release notes、prerelease 状态和可验证链接）；不是 Markdown Plan，也不是自动汇总所有已完成任务。与 Plan 的 `releasedIn` 可作为附加关联，但不能用待办事项编造已发布功能。
- 保持结构简洁：不是个人任务日志，也不是另一个项目管理数据库。

## Execution Tasks — single PR, one merge

- [ ] **T1 Inventory**：搜索并列出当前所有 `.agents/{plans,decisions,postmortems}` 文件，以及全仓对其的引用；记录每条旧决策/复盘的归宿和每个仍有效计划的当前状态。完成前不得删旧文件。
- [ ] **T2 JSON schema + validation**：定义 v1 Schema、正反例 fixture、唯一性/状态/引用/验收校验；同时提供人类友好的错误报告。运行时真正执行校验，不可只有静态 JSON Schema。
- [ ] **T3 Deterministic generator**：复用/扩展 `crates/lumilio-docgen`（或明确说明替代实现的理由），提供 `plans validate` / `plans generate` / `plans check`。生成 `docs/plans/*.md` 和 `web/src/data/roadmap.generated.json`，只输出 `primary: true` 的安全白名单字段到网站数据，按 `targetVersion` SemVer / `id` 固定排序、UTF-8、生成标识，不允许人手修改产物；用测试固定「最先三个」的顺序和无 primary 的边界行为。
- [ ] **T4 Full migration**：在同一分支完成正在进行、拟议及需留存历史知识的计划转换；把相关 ADR 决策和 Postmortem 的失败防线嵌入匹配 JSON 或稳定的实现规范；清理已经过时的记录。删除旧 `.agents/decisions/`、`.agents/postmortems/`、活跃 Markdown Plan 与其旧 README/backlog；所有历史引用一次性更新，引用原件时使用 Git commit permalink。
- [ ] **T5 Site + new routes**：重构 `web/src/components/Sections.astro`、`web/src/components/hero/plate.ts`、`Faceplate.tsx`、`Explorer.tsx` 与相关 Astro Props，使首页三个灰格及 Roadmap 摘要读取同一生成数据的前三个 `primary` Plan；保持九宫格固定布局、原有动画、响应式和键盘行为，但**删除**静态 `planned` 内容、`t.roadmap.items` 的重复手写事实及 curated Plan ID 映射。新增 `web/src/pages/roadmap.astro`，显示全部 primary Plan、目标版本、状态、公开摘要与必要的详情引用（计划版本不等于已发布）。新增 `web/src/pages/changelog.astro`，构建时读取真实 GitHub Releases 列表并显示版本、发布日期、发布说明、发布页链接；正式版与 prerelease 按发布时间或版本合理排序，但不按 major/minor/patch 分组。构建期请求失败应显示明确空态或最后有效的受控快照，不能伪造发布历史。首页与两独立页面提供导航与返回路径，继续使用现有设计语言、中文目录及必要的链接；页面数据不能在 UI 本地重新硬编码。
- [ ] **T6 Harness hard switch**：同时更新 `AGENTS.md`、`.agents/README.md`、`.agents/plans/README.md`、`.agents/skills/lumilio-exec-plan/SKILL.md` 和所有相关 Skills。最终指令只能要求 `Create/Update JSON → Execute → Validate → Outcome → Retain JSON`，不能出现旧“新建 ADR/删除 Plan”的主动指令。将 repo 的 Rust + GPUI + gpui-kit（gpui-component 本地 fork）、`web/` 与实际 CI 事实准确写入文档。
- [ ] **T7 CI + final audit**：让 docs/agent/plan-only PR 也运行 Plan 校验、确定性生成检测与站点构建检查；`just docs` 包含新验证。测试 SemVer 混合版本排序、三个 homepage primary 截断、少于三条时的空态、primary 切换、公开字段过滤，及 Changelog Releases API fixtures（正式版、预发布、空列表、限流/故障）。跑 `just docs`、`just web`、`just check`（或明确报告环境性阻塞），验证不存在旧生命周期指令、断开的 ADR/Postmortem 引用、双份可写 roadmap、内部字段泄漏。

## Acceptance

1. 合并后的 `main` **只有** JSON Plan 生命周期，没有两种可写格式，也没有 `decisions/`、`postmortems/` 或 `legacy/` 目录。
2. 所有需要继续使用的技术决策、事故防线和正在进行的工作都能从 JSON/权威代码规范查询，不靠不存在的旧路径。
3. 一个 Plan 可新建、推进、完成并留下 outcome；不会自动转成 ADR，也不会完成后删除。
4. 生成的 Markdown 与网站 JSON 可复现；手改生成文件、Schema 不合法、重复 ID 或不安全的公开数据导致 CI 失败。
5. 已实现 IA 继续从代码生成；首页三灰格和摘要只显示按 SemVer + ID 排序的前三个 primary Plan，`/roadmap` 显示全部 primary Plan；目标版本一律标记为「计划」，已完成 Plan 不被自动写成已发布。
6. `/changelog` 由 GitHub 已发布 Releases 构建，支持正式版和预发布，版本、时间与说明有来源；不能从未发布计划推断 Changelog。
7. 文档和 Agent Skills 与实际工具链一致；简单 Bugfix 不要求新建 Plan。
8. 对比原分支有完整旧引用迁移报告、生成数据校验结果和 CI / 本地测试结果。

## Merge Rule

**Do not merge documentation-only or partially migrated commits to `main`.** 所有 T1–T7 可在同一个 feature 分支逐步开发，但只能在功能、Schema、迁移、站点、AGENTS/Skills、CI 一起通过后，以一个完整 PR 合并。完成之前 Draft PR 始终不可合并。没有过渡期，没有 fallback 旧流程。
