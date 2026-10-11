---
title: 贡献规则
description: 人类、Coding Agent 和协作工作使用同一套项目规则。
---

每位 Project Maintainer 都使用这些规则。
规则适用于人类、Coding Agent，以及人类与 Coding Agent 的协作。

## 开始修改

1. 阅读本页。
2. 准备[开发环境](/docs/contribute/setup/)。
3. [选择工作方式](/docs/contribute/work-methods/)。
4. 按[修改与评审](/docs/contribute/change-and-review/)完成工作。

修改文件前，阅读 [AGENTS.md](https://github.com/EdwinZhanCN/LumilioCL/blob/main/AGENTS.md)。
验证使用 `justfile` 中的命令。
仓库文件提供当前的执行规则。

## 保持 crate 边界

主要依赖方向是 `lumilio-app → lumilio-ui → lumilio-core`。

| Crate | 职责 |
| --- | --- |
| `lumilio-core` | 启动器数据与操作；不依赖 GPUI |
| `lumilio-ui` | 页面、控件与 UI 状态 |
| `lumilio-app` | 启动与应用组装 |

领域逻辑应独立于 UI。
不要在 UI 线程执行阻塞 I/O。
增加新控件前，先检查已有控件。
核对 UI API 时，使用锁定版本的依赖源码。

## 使用源码证据

当前行为以代码为准。
使用 `docs/ia/paths/` 查找已实现的用户路径。
比较其他项目时，阅读公开的上游源码。
不要要求私有本地克隆或工具。

改编上游代码时，保留来源归属。
保留适用的许可证声明。
不要复制 Modrinth 品牌标识。
依赖归属见 [ATTRIBUTIONS.md](https://github.com/EdwinZhanCN/LumilioCL/blob/main/ATTRIBUTIONS.md)。

## 在 JSON 中记录工作

多步骤或跨会话工作使用 `.agents/plans/<slug>.json`。
小型本地修复不需要计划。
验收条件、决策和失败教训都写入 JSON 计划。
已完成和已取消的计划都保留。

修改计划后，运行 `just plans`。
不要修改生成的计划 Markdown 或 Roadmap 数据。
不要创建独立 ADR、Postmortem、归档或历史映射表。
已删除的记录通过 Git 历史查看。

## 使用项目称谓

所有执行项目工作的人员和 Coding Agent 都称为 `Project Maintainer`。
项目说明中不记录个人姓名或机器专属路径。
引用 Git 跟踪的路径或公开源码 URL。
保留许可证署名和公开仓库标识。
