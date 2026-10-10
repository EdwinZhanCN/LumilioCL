---
title: 编写文档
description: 使用 ASD-STE100 英文写作规则，并维护一致的中文与英文术语。
---

英文公开文档使用 ASD-STE100 Simplified Technical English。
完整规则和词典见[官方规范](https://www.asd-ste100.org/)。
中文采用相同的简短句子、单步指令和一致术语。
ASD-STE100 是英文规范，不是中文语言认证。

## 编写指令

1. 使用命令式表达。
2. 每个句子只写一个操作。
3. 英文操作句不超过 20 词。
4. 把条件放在操作之前。
5. 使用主动语态。
6. 同一事物使用同一术语。
7. 每个段落不超过六句。

英文说明句不超过 25 词。
词典中的词使用其批准的词义。
必要的软件名称作为技术术语。
不要为了缩短句子而移除必要信息。

## 技术术语

| 术语 | 含义 |
| --- | --- |
| Project Maintainer | 执行项目工作的人员或 Coding Agent |
| Coding Agent | 检查文件、修改代码和运行命令的软件 |
| repository | Git 项目及其跟踪的文件 |
| crate | Rust 包 |
| JSON plan | `.agents/plans/` 中保留的执行记录 |
| acceptance condition | 可检查的必要结果 |
| handoff | 交付修改和验证结果 |
| user path | 启动器中声明的操作及反馈 |
| regression test | 检测已发现缺陷的测试 |
| WASM plugin | 提议中的 WebAssembly 插件 |
| API | 软件组件之间定义的接口 |
| dependency | 其他软件包需要的软件包 |
| toolchain | 指定版本的编译器及相关工具 |
| UI | 用户操作应用的控件和显示界面 |
| CI | GitHub Actions 中的自动项目检查 |
| checksum | 用于核对文件内容的计算值 |
| pull request | 交给评审的 Git 修改 |
| fork | 上游项目的独立副本 |
| branch | Git 中具名的工作分支 |
| skill | 针对某类项目工作的仓库指令 |
| manifest | 描述包及其要求的文件 |

命令、API 名、文件名和 UI 标签应保持准确。
这些标识使用代码格式。
官方产品名称保持不变。

### 技术动词

英文技术动词的含义如下。

| 动词 | 含义 |
| --- | --- |
| build | 从源码生成应用 |
| compile | 把源码转换为可执行代码 |
| run | 启动程序、命令或检查 |
| install | 把软件放到所需位置 |
| clone | 创建 Git 仓库的本地副本 |
| commit | 在 Git 中记录一组修改 |
| push | 把 Git 提交传到远端仓库 |
| merge | 合并 Git 分支的修改 |
| block | 等待操作完成时，阻止调用代码继续 |

## 添加页面

1. 在 `web/src/content/docs/docs/` 添加中文页面。
2. 在 `web/src/content/docs/en/docs/` 添加对应英文页面。
3. 在 frontmatter 中填写 `title` 和 `description`。
4. 在 `web/astro.config.mjs` 的 sidebar 中添加页面。
5. 用当前源码核对技术描述。
6. 在仓库根目录运行 `just web`。
7. 检查桌面和移动页面。

两种语言使用相同的相对文件路径。
组件文案使用 `web/src/i18n/docs.ts`。
Starlight 界面翻译使用 `web/src/content/i18n/`。
侧栏标签使用 Starlight 的 `translations` 字段。

功能说明放在 `features/`。
未来的插件契约和开发流程放在 `wasm/`。
描述未完成功能前，先说明实现状态。
转发的 Release Notes 保留源语言，不自动翻译。

## 评审语言

核对句长和技术术语。
确认每条指令只有一个明确操作。
核对英文词义和词性。
自动句长检查不认证 ASD-STE100 合规。
Project Maintainer 还需评审语言。
