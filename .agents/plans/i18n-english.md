# 界面多语言，并补上英文

- Status: proposed

## Goal

启动器界面的文字从代码里的中文硬编码，变成可切换的语言目录。默认仍是简体中文，并提供完整英文。设置里可以选「跟随系统 / 简体中文 / English」，切换后当前窗口立即使用新语言，不必重启。

## Scope

- In：`lumilio-ui` 与 app 壳层里给人看的句子；设置页的语言选择；核心插件在设置里显示的名称和描述；缺译回退；英文目录与中文目录一起入库。
- Out：游戏日志、崩溃报告、Modrinth 项目正文（见 `discover-translation.md`）、版本号和项目 id、`docs/ia` 的中文路径注释、设计语言文档。不在这一轮增加第三种语言。

## 调研

界面字符串今天直接写在 Rust 里，例如账户页的「本地皮肤」和皮肤对话框的四个选项。没有 Fluent、gettext 或 `rust-i18n`。GPUI 0.3.7 没有自己的本地化层，字符串是在 `Render` 里取的，所以语言必须是渲染时能读到的应用状态。

Modrinth App 用 FormatJS 的 `defineMessages`，皮肤页的分组标题都有独立 message id（`3rd-party/modrinth/apps/app-frontend/src/pages/Skins.vue`）。那套绑定 Vue，不能搬过来。Rust 桌面端适合 Project Fluent：复数、日期和占位符由目录表达，译者改 `.ftl` 而不改 Rust。`fluent` crate 在 UI 侧加载两个 bundle；core 仍不依赖 UI，也不为了翻译去格式化句子。core 抛出的错误保持类型化，UI 把类型映射成 message id。

设计语言把中文交给系统字体，拉丁和数字用 Space Grotesk。英文不改变这个分工。缺 key 时先回退简体中文，再回退 key 本身，并在测试里失败，避免英文界面出现裸 id。

## References

- `crates/lumilio-ui/src/skin_dialog.rs`、`crates/lumilio-ui/src/live/accounts.rs`：当前硬编码中文
- `docs/design-language.md` §13：字体分工
- `3rd-party/modrinth/apps/app-frontend/src/pages/Skins.vue`：`defineMessages` 的粒度
- Project Fluent：<https://projectfluent.org/>

## Tasks

- [ ] T1：选定 `fluent` / `fluent-bundle`，在 UI 放 `locales/zh-CN` 与 `locales/en`。消息 id 用 `页面.部件.含义`。带数字的句子用 Fluent 复数，不用字符串拼接。
- [ ] T2：把现有 UI 与 app 壳层的可见字符串抽进中文目录，英文目录逐条补齐。插件清单里展示给设置页的名称和描述也进目录，按插件 id 查找。
- [ ] T3：语言偏好写入设置：跟随系统、`zh-CN`、`en`。跟随系统只认这两种，其余系统语言落到简体中文。切换时更新全局 bundle 并刷新窗口。
- [ ] T4：缺译测试。英文 bundle 必须覆盖中文 bundle 的全部 id；格式化参数不一致时测试失败。

## Validation

- 两个目录的 id 集合相等。
- 设置从中文切到英文后，账户、设置、发现、实例页的固定标题变为英文；再切回中文。
- 一条带数量的句子在英文使用复数规则。
- 游戏日志正文不经过这套目录。

## Open questions

- 插件协议里由插件临时生成、宿主无法预知的句子，这一轮仍显示插件原文。等 WASM 插件计划再定社区插件自己的语言包怎么挂上来。
