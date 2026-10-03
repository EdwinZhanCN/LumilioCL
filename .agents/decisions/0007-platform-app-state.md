# 0007 — 平台 App State 根目录与生命周期

- Status: proposed
- Date: 2026-09-30

## Context

ADR 0006 已建立 SQLite、独立 profiles 和共享 meta。现有单根解析尚未区分缓存、配置与恢复状态；Windows 使用 Roaming AppData；natives 没有平台维度。需要明确离线依赖、用户世界与可丢弃缓存的边界。

## Decision

提议采用 [docs/app-state.md](../../docs/app-state.md)：app 解析平台 data/config/state/cache 根，core 的 Layout 接收路径并统一应用自有路径定义。macOS data 保持 Application Support，Windows 改为 Local AppData，Linux 使用 XDG；LUMILIO_HOME 保留 data 根覆盖并将其他角色放子目录。

沿用 ADR 0006 的数据库、profile 和 meta；已安装依赖不按缓存清除。新增平台隔离、可恢复迁移和操作日志契约。在线秘密只进入系统凭据库。分阶段实施前不改变现有读写行为，也不宣称新目录已存在。

## Consequences

- Positive: 可预测的备份/清理语义；不漫游大型本机资源；支持明确的故障恢复。
- Negative / trade-offs: 多根 Layout 增加参数与迁移复杂度；缓存丢失可导致下载重新开始。
- Follow-ups: 按规范 P1/P2/P3 分别开实施计划并添加故障测试；在线凭据随认证功能实施。实施时由维护者确认本提案，再迁移当前行为。无需现在新增依赖。
