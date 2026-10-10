---
title: WASM 插件
description: 开始插件工作前，先了解当前限制。
---

## 当前状态

目前不能独立安装第三方 WASM 插件。
当前启动器包含内置原生插件。
不要把原生插件 API 当作已发布的 WASM 契约。

提议中的工作见 [WASM 插件计划](https://github.com/EdwinZhanCN/LumilioCL/blob/main/.agents/plans/wasm-plugin-registry.json)。
目标版本不是发布承诺。

## 后续文档

实现与验收完成后，本节将提供插件契约。
文档将包含：

- Manifest 与兼容规则。
- 宿主函数和插件权限。
- 构建、测试和安装流程。
- 错误、超时和取消行为。
- 包验证与分发。

当前项目工作遵循[贡献规则](/docs/contribute/)。
