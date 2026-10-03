# Modpack import behavior

Modrinth packs (`.mrpack`): a zip with `modrinth.index.json`, plus `overrides/`
and `client-overrides/` folders.

- The index must be format version 1, game `minecraft`, have a name, a
  `minecraft` dependency and at most one loader dependency
  (`fabric-loader`, `quilt-loader`, `forge`, `neoforge`); no loader means
  vanilla. Every file needs a safe relative path and a SHA-1.
- A path is safe when it is relative, `/`-separated, has no empty, `.` or `..`
  parts, and no backslash, colon or control character.
- Downloads are limited to https on trusted hosts (`cdn.modrinth.com`,
  `github.com`, `raw.githubusercontent.com`, `gitlab.com`), as the pack format
  requires; untrusted URLs are dropped, and a file left with none is an error.
  Mirrors expand the trusted URLs afterwards.
- Files marked unsupported on the client are skipped; optional ones are
  installed. Every download is verified by SHA-1 and size.
- Overrides are copied into the game directory; `client-overrides/` wins over
  `overrides/`. Entries that would escape the directory are ignored.
- Import builds the game folder (overrides, then downloads) in a staging folder
  and only then creates the instance and moves the folder into place; the
  library is not locked while downloading. If anything fails or is cancelled,
  nothing is left and no existing instance changes. See
  [staged instances](staged-instances.md).

已取消的 import 在读取/创建实例前返回 Cancelled；overrides 后、下载批次前再检查 token，防止空批次直接成功。暂存发布使半成品实例不可见，崩溃后由启动恢复补完或丢弃。
