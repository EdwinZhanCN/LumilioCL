# 统一日志视图

- Status: in_progress

## Goal

来源与级别下拉、共用满高阅读区、统一导出、分析弹窗与脱敏技术详情复制。

## References

- `3rd-party/modrinth/apps/app-frontend/src/pages/instance/logs/index.vue`
- `3rd-party/modrinth/packages/ui/src/layouts/shared/console/layout.vue`

## Tasks

- [x] 扩展历史日志、分析、导出接口。
- [x] 实现统一来源、筛选、分析弹窗与满高阅读区。
- [x] 更新 IA、回归测试。
- [x] just check。
- [x] 实机视觉验收：用户明确接手自行验证，停止代理窗口检查。

## Validation

历史压缩日志、迟到结果、报告全文、导出作用域、分析弹窗、缩放与实时跟随。

- core 压缩历史日志读取、完整导出、来源范围与脱敏测试通过。
- UI 来源切换、分析弹窗与技术详情复制、迟到结果、随高度伸缩测试通过；滚动测试发现小窗口阅读区不足，设置容器最小可用高度后聚焦测试通过。
- `just ia` 已更新；`cargo fmt --check`、`git diff --check` 通过。
- `just check` 首轮构建通过，现有 images 测试因沙箱禁止绑定回环端口失败。
- 沙箱外重跑停在 lumilio-ui 的 rustc；禁用增量编译并限制并发后仍停在该编译进程（约 2 秒 CPU 时间后长期等待）。未完成全量检查，不能宣称通过。
- 用户接手自行验证；已关闭本次卡住的编译与采样进程，避免占用验证资源。
- 临时数据与测试应用只在 `/tmp/lumilio-log-review`；测试库初次启动被 schema 0 导入流程清空，未完成真实日志页视觉检查。
