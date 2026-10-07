# 发现页的免费机器翻译

- Status: proposed

## Goal

发现页上的项目标题、简介、正文、图库说明、版本名称和更新日志，可以翻译成当前界面语言。翻译调用 SiliconFlow 上的免费翻译模型，密钥只进系统凭据库。默认关闭。失败、限流或未配置密钥时仍显示原文。这是启动器功能，不是插件。

## Scope

- In：发现搜索结果和项目详情里给人读的文字；按界面语言翻译；磁盘缓存；密钥、开关和「显示原文」；限流与模型不可用时的明确状态。
- Out：翻译启动器自己的界面（那是 `i18n-english.md`）、作者名、文件名、版本号、许可证 id、分类 id、日志和崩溃报告；自动改用付费模型；把正文用于模型训练。

## 调研

发现页的文字来自内容协议，不是 UI 目录。`SearchHit` 有 `title`、`description`、`categories`；`Project` 另有 `body` 和 `gallery` 的标题与说明；`Version` 有 `name` 和 `changelog`（`crates/lumilio-plugin-api/src/content.rs`）。作者名、slug、下载量和版本号不应翻译。

SiliconFlow 的对话接口与 OpenAI 兼容：`POST https://api.siliconflow.cn/v1/chat/completions`，国际线路是 `https://api.siliconflow.com/v1`。响应里用 `x-siliconcloud-trace-id` 排错。限流正文会说明 TPM，过载代码是 50505（<https://docs.siliconflow.cn/cn/api-reference/chat-completions/chat-completions>）。

专用翻译模型是腾讯混元 `Hunyuan-MT-7B`，模型 id 使用 `tencent/Hunyuan-MT-7B`。它做 33 种语言互译，SiliconFlow 在 2025-09-18 上架（<https://www.siliconflow.com/models/hunyuan-mt-7b>）。2026-08-21 的价目快照把它标为免费，但免费模型需要实名，并且有固定速率限制；价目会变。实现时核对当时的价目页。若该 id 不再免费，设置里显示「当前模型已不是免费模型」，停止发送，绝不静默换成付费聊天模型。

密钥按 ADR 0020 进系统凭据库。设置里只保存「已配置 / 未配置」、线路（中国或国际）和模型 id。日志不记录密钥、令牌和完整提示词。

翻译默认关闭。打开时设置说明：项目文字会送到 SiliconFlow。目标语言等于界面语言；原文语言已经是目标语言时不请求。缓存键是目标语言、模型 id 和原文哈希，放在启动器缓存目录，可以一键清除。

搜索页把一页标题和短简介合成一次请求，用稳定分隔和序号回填，避免每个卡片一次往返。详情正文和更新日志单独请求。回填失败则这一条保持原文，不用错位的译文。

## References

- `crates/lumilio-plugin-api/src/content.rs`：`SearchHit`、`Project`、`GalleryImage`、`Version`
- SiliconFlow Chat Completions：<https://docs.siliconflow.cn/cn/api-reference/chat-completions/chat-completions>
- Hunyuan-MT-7B：<https://www.siliconflow.com/models/hunyuan-mt-7b>
- ADR 0020

## Tasks

- [ ] T1：core 增加翻译客户端。请求只含待译文本和目标语言。解析失败、429、50505、网络失败各自返回原因。测试用脚本化服务器，不访问真实 SiliconFlow。
- [ ] T2：凭据库存密钥。设置页有开关、线路、密钥状态、清除缓存。开关默认关。模型 id 固定为 `tencent/Hunyuan-MT-7B`，不提供随意填写付费模型的框。
- [ ] T3：发现列表翻译 `title` 和 `description`。详情翻译 `body`、图库标题和说明、版本 `name` 与 `changelog`。每条能切回原文。缓存命中不再请求。
- [ ] T4：界面语言切换后使用对应缓存；没有缓存时重新请求。关闭开关立即回到原文，不删缓存。

## Validation

- 脚本化响应覆盖成功回填、条数不匹配、限流、过载和空密钥。
- 缓存测试证明相同原文不第二次出网，清除后会再请求。
- 诊断和日志夹具里不出现密钥。
- 作者名、文件名、版本号保持原文。
- 实机：用维护者自己的免费密钥打开一页英文项目，标题和简介变为中文，失败时卡片仍可打开。

## Open questions

- 混元翻译的推荐提示词以实现当周的模型卡为准。计划不把某一段提示词写死成协议。
- 分类的显示名若来自我们自己的中英目录，不送去翻译。只有接口直接给的自然语言才进入请求。
