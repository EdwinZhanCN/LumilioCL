# 实例的截图墙

- Status: proposed

## Goal

游戏页多一个「截图」页签：缩略图网格读 `<游戏>/screenshots`，点开看大图，复制、在访达中显示、删除（确认）。

## Scope

- In：core `screenshots`（列表、缩略图后台生成与缓存、删除）；服务层；UI 页签与查看器。
- Out：跨实例总览；设为背景；导出。

## References

- `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/versions/`（若有截图相关）；Modrinth 没有对应功能。

## Tasks

- [ ] T1: core 列表与缩略图缓存（启动器缓存目录）+ 测试。
- [ ] T2: 服务层读取与删除。
- [ ] T3: UI 页签、网格、查看器；app 接线。
- [ ] T4: IA 路径、四项检查。

## Open questions

- 缩略图解码用哪个 crate（`image` 的最小特性）。
