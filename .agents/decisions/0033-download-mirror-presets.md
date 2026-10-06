# 0033 — 下载镜像预设按需追加

- Status: accepted
- Date: 2026-10-06

## Context

下载设置已经支持地址前缀规则和官方 / 镜像优先顺序，但人必须手写规则。维护者要求参考 HMCL 预设镜像。HMCL 的 `BMCLAPIDownloadProvider.java` 包括 BMCLAPI、MCIM 后备规则和腾讯 Maven Central 镜像。

## Decision

在 core 提供三个独立的预设：BMCLAPI（游戏资源、Forge / NeoForge / Fabric、authlib-injector），MCIM（Modrinth / CurseForge），腾讯 Maven（Maven Central）。设置页按需添加，追加缺失的完整规则对，不覆盖已有规则、不改变既有候选顺序，也不自动打开镜像优先。默认仍只访问官方地址；规则仍通过原有编辑入口修改或清空。

预设前缀以路径边界结束，避免把相似域名当成官方地址；Minecraft 资源对象同时覆盖 HTTP 和 HTTPS。使用已有 SourceChain 保留官方回退，不新增测速、地域判断或另一套持久化格式。

添加入口只发送预设身份；core 在设置锁内合并最新保存的规则，避免连续添加不同预设时用陈旧界面快照覆盖先前的添加。

## Consequences

不需要手写常用规则；重复添加无效，自定义镜像仍排在新加入的预设之前。预设不是系统 HTTP / SOCKS 代理，服务地址也不代表实时可用性保证。MCIM 沿用当前全局源优先开关，未引入 HMCL 的独立后备优先策略。

## Shipped

core 预设与规则合并、下载设置的三个添加入口、生成的用户路径；测试覆盖地址映射、官方回退、配置持久化、自定义候选顺序和添加按钮的交互。
