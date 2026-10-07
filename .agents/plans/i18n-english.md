# 界面多语言，并补上英文

- Status: in_progress

## Goal

启动器界面的文字从代码里的中文硬编码，变成可切换的语言目录。默认仍是简体中文，并提供完整英文。设置里可以选「跟随系统 / 简体中文 / English」，切换后当前窗口立即使用新语言，不必重启。

## Scope

- In：`lumilio-ui` 与 app 壳层里给人看的句子；设置页的语言选择；核心插件在设置里显示的名称和描述；缺译回退；英文目录与中文目录一起入库。
- Out：游戏日志、崩溃报告、Modrinth 项目正文（见 `discover-translation.md`）、版本号和项目 id、`docs/ia` 的中文路径注释、设计语言文档。不在这一轮增加第三种语言。

## 调研

界面字符串今天直接写在 Rust 里，例如账户页的「本地皮肤」和皮肤对话框的四个选项。GPUI 0.3.7 没有自己的本地化层，字符串是在 `Render` 里取的，所以语言必须是渲染时能读到的应用状态。

规模（2026-10-07，不含测试，按含中文的字符串字面量行数计）：`lumilio-ui` 约 1270 行、94 个文件；`lumilio-app` 约 140 行、13 个文件；`lumilio-core` 约 40 行、15 个文件（错误与说明文字）；核心插件中 crash-analyzer 约 44 行、litematica 约 29 行、discord 约 10 行。一次性全抽完不现实，要按页推进，并用测试挡住新增的硬编码。

`forks/gpui-component` 自带 `rust-i18n`（`locales/ui.yml`，含 `en` 与 `zh-CN`），由 `gpui_component::set_locale` 切换。本仓库从没调用过它，所以输入框右键菜单、日历这类组件自带文字现在按 `rust-i18n` 的默认 `en` 显示。语言切换必须同时设置它。

core 里的中文是错误和说明句子，crash-analyzer 的诊断结论也是插件生成的中文句子。英文界面下它们仍会是中文，除非 core 错误改成类型化、插件输出改成 id 加参数。

Modrinth App 用 FormatJS 的 `defineMessages`，皮肤页的分组标题都有独立 message id（`3rd-party/modrinth/apps/app-frontend/src/pages/Skins.vue`）。那套绑定 Vue，不能搬过来。Rust 桌面端适合 Project Fluent：复数、日期和占位符由目录表达，译者改 `.ftl` 而不改 Rust。`fluent` crate 在 UI 侧加载两个 bundle；core 仍不依赖 UI，也不为了翻译去格式化句子。core 抛出的错误保持类型化，UI 把类型映射成 message id。

设计语言把中文交给系统字体，拉丁和数字用 Space Grotesk。英文不改变这个分工。缺 key 时先回退简体中文，再回退 key 本身，并在测试里失败，避免英文界面出现裸 id。

## References

- `crates/lumilio-ui/src/skin_dialog.rs`、`crates/lumilio-ui/src/live/accounts.rs`：当前硬编码中文
- `docs/design-language.md` §13：字体分工
- `3rd-party/modrinth/apps/app-frontend/src/pages/Skins.vue`：`defineMessages` 的粒度
- Project Fluent：<https://projectfluent.org/>

## Tasks

- [x] T1：`i18n-embed`（`fluent-system`、`desktop-requester`）加 `i18n-embed-fl`。目录在 `crates/lumilio-ui/i18n/<语言>/lumilio-ui.ftl`，`crates/lumilio-ui/i18n.toml` 把回退语言定为 `zh-CN`。`rust-i18n` 只留给 gpui-component 内部，它不支持复数（longbridge/rust-i18n#65）。消息 id 用 `页面-部件-含义`（Fluent id 不能有点号）。
- [ ] T2：把现有 UI 与 app 壳层的可见字符串抽进目录，一次一页。已完成：设置页；导航栏与外壳；首页（含英雄区）；游戏库（含合集、导入选择、app 里游戏库的提示）。剩余按 `crates/lumilio-ui/src/i18n/hardcoded.txt`（约 1020 行、75 个文件），下一页是发现。英文由 `dsh` 按 skill `lumilio-i18n` 的提示词补，人再审一遍。插件清单里展示给设置页的名称、描述和设置项也进目录，按插件 id 查找。app 已有 `crates/lumilio-app/i18n.toml`（`domain = "lumilio-ui"`，指向 UI 的目录），因为 `fl!` 按调用方 crate 找配置。
- [ ] T2b：core 错误里给人看的句子改为类型化，UI 映射成 message id；crash-analyzer 等核心插件的结论改为 id 加参数，或先记为这一轮的已知缺口。
- [x] T3：`Preferences.language`（`System` / `SimplifiedChinese` / `English`），设置·通用里的分段。跟随系统取系统语言列表里第一个中文或英文，都没有则简体中文。`i18n::apply_language` 换目录、调 `gpui_component::set_locale` 并刷新窗口；app 启动时先按系统语言，读到保存的选择后再换。
- [x] T5：切换语言后，把文字存进实体的控件要重新取词。`i18n::generation()` 每次切换加一；`LiveControls` 记下自己的代号，外壳渲染时发现不同就 `relabel`：换占位符、换下拉选项并保留所选的项和已输入的文字，加载器列表标为过期由外壳重建。别处新的这类控件照此办理。
- [x] T4：`i18n/tests.rs`：两个目录 id 与参数一致、英文复数、参数两侧无方向隔离符、系统语言选择；`hardcoded_chinese_only_shrinks` 只许清单变短（`just hardcoded-chinese` 重写）。`tests/english.rs` 在单独进程里切到英文再切回。已验证：错 id 编译失败，新增中文字面量让清单测试失败。

## Validation

- 两个目录的 id 集合相等。
- 设置从中文切到英文后，账户、设置、发现、实例页的固定标题变为英文；再切回中文。
- 一条带数量的句子在英文使用复数规则。
- 游戏日志正文不经过这套目录。

## Learned

- 无参数的消息由 `tr!` 返回 `&'static str`：每个（语言，id）格式化一次并留到进程结束，总量受目录大小限制。这样 `FieldSpec`、`kit::switch` 等收 `&'static str` 的接口不用改签名；`tr_all![…]` 同理给出 `&'static [&'static str]`，用于分段和选项。带参数的消息返回 `String`。
- `fl!` 要求调用处给的参数与中文消息用到的参数完全一致；英文目录又被测试要求参数相同。所以句子要设计成两种语言用同一组参数，例如「-Da 等 3 项」与 "3 items: -Da, …" 都用 `first` 和 `count`。
- Fluent 会去掉值两端的空白；带空格的分隔符要写成 `{", "}`。
- Fluent 默认在参数两侧插入 Unicode 方向隔离符，加载后用 `set_use_isolating(false)` 关掉。
- 构建时 `rust-embed` 开 `debug-embed`，改目录会让 crate 重新编译、`tr!` 重新检查。
- 已经建好的实体里存下的文字（输入框占位符、窗口标题）不会因刷新而换语言，要等它们重建。

## Open questions

- 插件协议里由插件临时生成、宿主无法预知的句子，这一轮仍显示插件原文。等 WASM 插件计划再定社区插件自己的语言包怎么挂上来。
