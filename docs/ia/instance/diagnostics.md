# 游戏页 · 诊断

[返回游戏页](README.md) · ARCH：Instance.Diagnostics（Problems · Logs · Files） · 流程：L-DIAG-01、L-PLAY-01 · HMCL：H-PLAY-09/10、H-INSTANCE-10

## 信息层级

```
L4  问题 | 日志 | 文件
问题  列表：严重度点 · 一句话 · 原因 · [解决操作]（与概览“需要留意”同源，这里列全部）
日志  L4b  来源：[正在运行 / 最新日志 / 崩溃报告 2026-09-27 ⌄]   级别：全部 · 错误 · 警告 · 信息   [搜索日志]   [复制] [导出…] [⤢ 展开]
      L5   等宽日志视图（虚拟滚动，按级别着色，跟随末尾；崩溃报告上方先显示识别出的原因卡片）
文件  L4b  面包屑 游戏目录 / config /     [搜索文件]   [在访达中显示]
      L5   表格：名称 · 大小 · 修改时间 · ⋯（在访达中显示 · 复制路径）
```

## 用户路径

已实现的路径由代码里的 `// ia[instance.diagnostics]` 注释生成，见 [paths/instance.diagnostics.md](../paths/instance.diagnostics.md)（约定见 ADR 0019）。目前没有未做的路径。

## 范围外

- 文件编辑、NBT 编辑（H-EXTRA-06）：⏸。
- 线程 dump：⏸。
