# Launcher service behavior

`LauncherService` is what a front end talks to. It owns the instance store,
settings, task board and activity log under one launcher root.

- **Library**: instances and collections as records; delete is journaled and
  recoverable ([deletion](deletion.md)); `meta/` is never touched. Opening the
  service settles earlier damage ([recovery](recovery.md)).
- **Home**: diagnoses at most the eight most recently played instances (file
  verification is not free), then applies the Continue / Recent / Needs
  Attention rules from `attention.md`.
- **Create**: no game version → newest release from the catalog; loader without
  a version → the recommended one for that game. Loaders that cannot launch
  (Forge, NeoForge) are refused before anything is stored. If the catalog or
  loader list cannot be fetched, nothing is created. Creating only registers the
  record (`installed` false); a new id never adopts a profile folder already on
  disk.
- **Install instance**: `install_instance` downloads and verifies the release,
  libraries, assets, natives and loader profile without choosing Java or
  starting a process. It is an Install task in Activity and `cancel_task` stops
  it, including while the manifest is being fetched. Only a complete install sets
  `installed`; failure or cancel leaves it false and a retry is a new run. It
  records no play session.
- **Launch**: fetches the catalog only when the release (and loader profile)
  are not already on disk; an installed instance launches with no network, and
  a missing catalog is tolerated when the vanilla release is present. Player is
  the selected offline account (with its custom id, if any). With none selected
  the launch fails with `NoAccount` before anything is prepared; there is no
  stand-in player. The launch defaults reach the real process: the wrapper
  words come first, then Java; the window (`--width`/`--height`) and
  `--fullscreen` follow the game arguments unless one is already there, and the
  user's game arguments come last. The environment carries the user's
  variables plus `INST_ID`, `INST_NAME`, `INST_DIR` (the game folder),
  `INST_JAVA`, `INST_MC_VERSION` and `INST_LOADER`. A pre-launch command runs
  through the system shell in the game folder before the game starts; a non-zero
  exit or being stopped fails the launch and no process is created. A post-exit
  command runs after the game, for at most 60 seconds, and only its failure is
  logged. On return the instance is marked
  installed and the session is recorded (history + totals). A launch that never
  produced a process (preparation failed, or was cancelled) is recorded in
  history as *failed to prepare* / *cancelled* without play time. While a launch
  is in progress `state/sessions/<id>.json` exists; if the launcher itself dies
  the next start records *interrupted* and reports `SessionInterrupted`. The game
  is never re-attached, killed or restarted from that marker.
- **Search**: Modrinth search, unchanged from `discover.md`.
- **Install content**: newest compatible version for the instance's game
  version (and loader for mods) into `profiles/<id>/game/<folder>/`, verified by
  size and SHA-1; records a history change. No compatible version → an error
  and nothing is written. Every install is a task on the board and ends as a
  finished entry (succeeded / failed) in the activity log.
- **Install modpack**: downloads the newest `.mrpack` to `downloads/`, imports
  it as a new instance (rollback rules in `modpack.md`), deletes the download.
- **Java**: conventional locations plus the launcher's `runtimes/` and any
  configured extra roots; `with_runtime_roots` replaces the conventional ones.

- **Instance details/settings**: `instance(id)` reads the registered record even
  when its profile is missing. Live Library management opens that stable ID;
  rename and memory saves use separate requests. The form preserves unsaved
  drafts on failure, and results update the initiating view entity. Saving names
  refreshes Library/Home; saved memory applies to future launches. Clearing memory
  overrides preserves Java paths and JVM arguments. Current live UI exposes
  Overview and Settings.Performance; other Instance sections still need wiring.

## 操作冲突：首个服务守卫

单 LauncherService 内，同实例启动、内容安装和删除互斥，竞争请求返回 InstanceBusy。启动从冻结输入之前到进程监督及会话记录完成持有目标；安装持有到文件/history 提交返回；删除持有到目录操作返回。列表、收藏、改名和下次启动的设置仍可使用。错误及 future drop 释放实例租约，mutex 不跨 await。

此守卫尚不覆盖不同 LauncherService/进程，也不协调不同实例对共享 meta 的写入。根锁、共享资源锁、任务中断终态及可恢复删除仍由 0015/后续计划推进，不能将当前实例互斥视为完整 AC-RACE-02。

## 数据根写入所有权

LauncherService 在打开 store/settings 前获取 locks/writer.lock 的非阻塞 OS 独占锁，服务生命周期内持有；第二个 service/进程返回 RootBusy。锁文件不会删除，进程退出后由 OS 释放。不同根不互斥；直接低层 store 和外部工具不受此合作锁自动约束。

真实应用在 UI 循环前打开 backend；目录打开失败（包括锁冲突）以 stderr 原因和退出码 1 结束，不回退到示例界面。当前尚无专门恢复 UI。

## 显式取消与提交结果

内容/整合包安装的只读 chain/client/version 查询支持 token 取消，包括等待 settings 锁。TransferError 与 ModpackError 的 Cancelled 保留为 ServiceError::Cancelled，Activity 记录 Cancelled；future 被直接 drop 则仍是 Failed/interrupted。

下载自己控制取消与发布边界，成功返回后不按 token 迟到状态重分类已完成的文件提交。整合包在创建前检查取消，并在 overrides 后、download batch 前再检查，空文件列表不绕过取消。Activity 页已有取消按钮（`LiveIntent::CancelTask`）；完整崩溃恢复仍待接入。

## 只读诊断入口

`history(id)` 返回按时间从旧到新的事件和跳过的坏行数；`problems(id)` 对该实例做一次检查（与 Home 同源）；`logs(id)` 返回 `latest.log` 末尾最多 64 KB 和崩溃报告列表；`crash_report(id, file)` 返回报告文本（最多 256 KB）和识别到的原因，非纯文件名被拒绝。它们都不取实例租约，所以游戏运行中也能读。

## Direct start (quick play)

`launch_world` starts the game and goes straight into a saved world; an
instance may also carry a world or a server as its own direct-start choice, used
by an ordinary launch. The release decides: the game-argument rules for
`has_quick_plays_support`, `is_quick_play_singleplayer` and
`is_quick_play_multiplayer` are only satisfied for the chosen target, so a
version that declares none gets no such arguments. A world must be a folder
under `saves/` or the launch stops with `QuickPlayWorldMissing`; a release that
cannot start in a world stops with `QuickPlayUnsupported` instead of starting
at the menu. For a server on a release without direct start, `--server` and
`--port` are used. Installing never checks the target.

## Changing the version and repairing

`change_runtime` changes an instance's game version and/or loader under the
instance lease. The new combination is prepared first, as the instance would be
on that version: shared files go into `meta/` under their own release folder
(nothing another instance uses is rewritten) and a Forge or NeoForge installer
runs. Only after all of that succeeded is the record changed, in one library
transaction that also marks it installed. A download failure, a cancel, or a
library that cannot be written all leave the instance on its old combination,
still launchable offline. Worlds, mods and settings are never touched (mods
for another version or loader may stop working; the dialog offers a snapshot
first). The same combination again is refused, a loader needs its version, and
the change is recorded in the history as `GameVersionChanged` and in Activity
as an update.

`repair_instance` runs the same verification a launch does, with its own
Activity category and history entry: files that are missing or whose size or
checksum is wrong are fetched again, natives are rebuilt, and a Forge or
NeoForge patched client is rebuilt when missing. An intact game needs no
network and changes nothing.
