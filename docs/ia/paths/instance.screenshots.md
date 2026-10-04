<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；
     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->
# 游戏页 · 截图 · 已实现的用户路径

表里每一行都有对应的实现；没有的就是没做。

| 操作 | 层 / 组件 | 结果与反馈 | 备注 | 实现 |
|---|---|---|---|---|
| 刷新截图 | L2 次要「刷新」 | 重新读取 screenshots 文件夹；游戏里新拍的在游戏结束后也会自动出现 |  | `lumilio-ui/src/instance_detail/panels/screenshots.rs` |
| 打开截图文件夹 | L2 次要「打开文件夹」 | 在访达中显示 screenshots 文件夹 |  | `lumilio-ui/src/instance_detail/panels/screenshots.rs` |
| 查看截图 | 缩略图卡片 → 大图弹窗 | 看大图，可前后翻、复制图片、在访达中显示、删除（先确认） |  | `lumilio-ui/src/instance_detail/panels/screenshots.rs` |
| 复制截图 | 大图弹窗「复制图片」 | 图片进剪贴板，toast“已复制图片” |  | `lumilio-ui/src/instance_detail/panels/screenshots.rs` |
| 在访达中显示截图 | 大图弹窗「在访达中显示」 | 打开 screenshots 文件夹并选中该文件 |  | `lumilio-ui/src/instance_detail/panels/screenshots.rs` |
| 删除截图 | 大图弹窗「删除」→ 警告弹窗 | 删除文件，之后不能找回；游戏运行时也可以删 |  | `lumilio-ui/src/instance_detail/panels/screenshots.rs` |
