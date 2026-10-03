<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 游戏页 · 内容 · 已实现的用户路径

表里每一行都有对应的实现；没做的路径写在同目录手写的页面文档里。

| 操作 | 层 / 组件 | 结果与反馈 | 编号 | 备注 | 实现 |
|---|---|---|---|---|---|
| 浏览并安装 | L4a 主要 → 发现页（类型预选） | 安装完回到本页可见 | L-CONT-01 |  | `lumilio-app/src/live.rs` |
| 打开项目页 | 点标题（仅已识别） | 打开该项目的发现页详情（进历史） | H-CONTENT-05 |  | `lumilio-ui/src/instance_content.rs` |
| 更新单个 | 行内「更新」→ 切换版本弹窗，默认选最新兼容 | 下载 → 校验 → 替换旧文件 → 重扫 → 写历史；停用的保持停用 | H-CONTENT-07 |  | `lumilio-ui/src/instance_content.rs` |
| 切换版本 | 行内 ⇆ → 版本弹窗（仅已识别的文件） | 下载所选版本 → 校验 → 替换旧文件 → 重扫 → 写历史；停用的保持停用 | H-CONTENT-06/07/08 |  | `lumilio-ui/src/instance_content.rs` |
| 启用 / 停用 | 行内开关 | 即时生效；游戏运行中拒绝并 toast“游戏运行时不能改” | H-CONTENT-03 |  | `lumilio-ui/src/instance_content.rs` |
| 删除 | 行内 🗑 → 警告弹窗 | 删除文件、写历史、toast | H-CONTENT-04 |  | `lumilio-ui/src/instance_content.rs` |
| 在访达中显示 | 行 ⋯ 菜单 | 打开并选中文件 | H-CONTENT-05 |  | `lumilio-ui/src/instance_content.rs` |
| 复制链接 | 行 ⋯ 菜单（仅已识别） | 复制 Modrinth 项目链接，toast“链接已复制” | H-CONTENT-05 |  | `lumilio-ui/src/instance_content.rs` |
| 识别来源 | 进入内容标签时自动 | 按 SHA-1 查 Modrinth：图标、项目名、作者、版本、项目链接；离线时照常列出，未识别的没有切换版本键 | H-CONTENT-05 | 识别结果不缓存 | `lumilio-ui/src/instance_content.rs` |
| 切换子分类 | L4a 分段：Mod / 资源包 / 光影 | 列表切换；搜索与筛选各分类分别记住 | H-CONTENT-01 |  | `lumilio-ui/src/instance_content.rs` |
| 添加本地文件 | L4a 次要「添加文件」→ 选文件（拖入内容页没做） | 冲突逐项报告，不静默覆盖同名异内容的文件 | H-CONTENT-02/09 |  | `lumilio-ui/src/instance_content.rs` |
| 筛选 | L4b 分段：全部 / 有更新 / 已停用 / 未识别 | 视图状态 | H-CONTENT-01 |  | `lumilio-ui/src/instance_content.rs` |
| 全部更新 | L4b「全部更新（N）」→ 确认弹窗 | 逐项执行，部分成功分别报告；进度在动态 | H-CONTENT-07 | 弹窗内逐项取消勾选没做 | `lumilio-ui/src/instance_content.rs` |
| 刷新 | L4b ↻ | 重新扫描目录并重新识别 | H-CONTENT-01 |  | `lumilio-ui/src/instance_content.rs` |
| 批量启用 / 停用 / 删除 | 选中行后出现的批量栏 | 逐项结果；删除先确认 | H-CONTENT-03/04 |  | `lumilio-ui/src/instance_content.rs` |
| 搜索 | L4b 搜索框 | 按名称过滤（视图状态） | H-CONTENT-01 |  | `lumilio-ui/src/instance_content.rs` |
