# LumilioCL Roadmap

`ARCH.md` 与 `docs/architecture.md` 是范围和边界的权威；本文只记录阶段和当前位置。
每个计划的交付见 `.agents/plans/history.md`。

## 已完成

- **Phase 0 — 工程骨架**：计划/决策/复盘/技能的生命周期，core / UI / app 依赖方向。
- **Phase 1 — 工作区**：三个 crate；core 可独立测试。
- **Phase 2 — 导航壳**：ARCH 每个节点都有可达页面和空/加载状态。
- **Phase 3–7 — 领域切片**：版本目录与安装、Forge/NeoForge、账户与 Microsoft 登录、Modrinth、内容/世界/快照、
  复制与整合包、可恢复删除与启动恢复、操作协调（计划 0005–0021）。
- **Phase 8 — ARCH 页面接入**：各页面接入真实 core，托管 Java、完整备份、从其他启动器导入、共享资源回收
  （计划 0022–0035）。

## 当前

- 待维护者验收：Microsoft 真实登录（0031）、更换游戏版本（0030）、各页面视觉验收。
- 待维护者决策：ADR 0007、0008（仍是 proposed，只部分落地）；离线皮肤是否做（ADR 0018）。

## 未排期

第三方认证、CurseForge 整合包、启动器自动更新、整合包更新、恢复模式界面、数据迁移、跨卷删除。
