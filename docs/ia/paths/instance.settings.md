<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见技能 lumilio-ia-paths。 -->
# 游戏页 · 设置 · 已实现的用户路径

表里每一行都有对应的实现；没有的就是没做。

| 操作 | 层 / 组件 | 结果与反馈 | 备注 | 实现 |
|---|---|---|---|---|
| 用这个游戏的值替换预设 | 选项 · 待决定列表冲突说明旁的「用这个游戏的值替换…」→ 警告弹窗 | 预设里同一项也被改过时，看过预设的新值、确认后才替换；其他游戏下次启动才收到 |  | `lumilio-ui/src/game_settings/review.rs` |
| 处理游戏里改过的选项 | 选项 · 待决定列表勾选后：仅此游戏 / 更新预设 / 改回预设值 | 一起处理所选各项；更新预设前检查预设是否也改过，冲突时保留选择并标出预设的新值 |  | `lumilio-ui/src/game_settings/review.rs` |
| 内存 | 设置 · 性能组，「内存」区 [编辑] 弹窗（最小 / 最大） | 留空跟随默认；性能组另有只读的“本机内存 · 推荐最大”一行 |  | `lumilio-ui/src/instance_detail/performance.rs` |
| 选项 | 设置 tab 的分段「选项」 | 显示游戏内「选项」里的设置（按键、音量、画面、聊天、辅助功能、语言），游戏关着时可在这里改 |  | `lumilio-ui/src/instance_detail/settings/choices.rs` |
| 窗口大小、全屏 | 设置 · 游戏组，值 + [编辑] 弹窗 | 宽高一起填（留空跟随默认）；全屏 关 / 开 / 跟随默认；恢复默认＝移除覆盖 |  | `lumilio-ui/src/instance_detail/settings/groups.rs` |
| 进入游戏后 | 设置 · 游戏组，值 + [编辑] 弹窗 | 保持 / 隐藏启动器 / 跟随默认 |  | `lumilio-ui/src/instance_detail/settings/groups.rs` |
| 直接进入（Quick Play） | 设置 · 游戏组，值 + [编辑] 弹窗：类型 + 目标 | 启动后直达世界或服务器；版本不支持单人世界时启动会说明原因；世界页的「进入」用同一个能力 |  | `lumilio-ui/src/instance_detail/settings/groups.rs` |
| 更换游戏版本 / 加载器 | 设置 · 运行时，「游戏版本」「加载器」行的 [更换…] → 弹窗（加载器分段 + 版本选择器，起点是游戏现在的组合） | 警告“装好的 Mod 可能不兼容，先建快照” + [先建快照]；与现在相同时不能提交；失败回到旧组合 |  | `lumilio-ui/src/instance_detail/settings/groups.rs` |
| 修复游戏文件 | 设置 · 运行时，「游戏文件」行的 [修复] | 逐个核对游戏文件，缺的或损坏的重新下载；世界、Mod 和设置不会被改动；后台任务 |  | `lumilio-ui/src/instance_detail/settings/groups.rs` |
| Java 参数 | 设置 · Java 组，值 + [编辑] 弹窗 | 多行文本，每行一个参数 |  | `lumilio-ui/src/instance_detail/settings/groups.rs` |
| 游戏参数 | 设置 · 高级组，值 + [编辑] 弹窗 | 来源（跟随默认 / 自己设置）+ 多行 |  | `lumilio-ui/src/instance_detail/settings/groups.rs` |
| 环境变量 | 设置 · 高级组，值 + [编辑] 弹窗 | 来源 + 每行 名称=值 |  | `lumilio-ui/src/instance_detail/settings/groups.rs` |
| 启动前 / 包装 / 退出后命令 | 设置 · 高级组，值 + [编辑] 弹窗 | 三个输入：留空跟随默认，填 - 表示这个游戏不使用；说明可用变量 |  | `lumilio-ui/src/instance_detail/settings/groups.rs` |
| 指定 Java | 设置 · Java 组，下拉「自动（推荐）/ 已发现的 Java / 自定义路径…」 | 选中即保存；值下方写出下次启动用哪个 Java，没有合适的时写出启动时会下载的版本；已停用的 Java 不在列表里 | java-management T4 | `lumilio-ui/src/instance_detail/settings/java.rs` |
| 自定义 Java 路径 | 设置 · Java 组，选「自定义路径…」后出现的路径行 + [编辑] 弹窗 | 填 java 程序或 JDK 文件夹；保存时检查路径存在；恢复默认＝回到自动 |  | `lumilio-ui/src/instance_detail/settings/java.rs` |
| 复制自定义 Java 路径 | 设置 · Java 组，路径旁的复制键（路径太长时中间省略，悬停看完整路径） | 完整路径放进剪贴板，toast“已复制路径” |  | `lumilio-ui/src/instance_detail/settings/java.rs` |
| 选择预设 | 设置 › 选项，「预设」行的「更换…」→ 弹窗：不使用预设或某个预设，并预览下次启动会改变的选项 | 选了预设后下次启动应用它共享的选项；改回不使用时保留现在的选项；游戏运行、有待决定的修改或上次会话未确认关闭时不能换 |  | `lumilio-ui/src/instance_detail/settings/options.rs` |
| 确认游戏已关闭 | 设置 › 选项，上次会话中断时的说明和「游戏已关闭」 | 玩家确认后才比较游戏文件和上次应用的值，找出游戏里改过的选项；游戏还开着时不要点 |  | `lumilio-ui/src/instance_detail/settings/options.rs` |
| 修改选项 | 设置 › 选项的分组表：开关当场切换，其他选项「编辑」→ 弹窗（滑块和数值 / 分段 / 可搜索列表） | 游戏自己的选项当场写进 options.txt；预设共享的选项改成“仅此游戏”，下次启动生效，弹窗里可「跟随预设」改回；游戏运行时不能改 |  | `lumilio-ui/src/instance_detail/settings/options.rs` |
