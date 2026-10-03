# LumilioCL Architecture

This is the stable system map for LumilioCL. `ARCH.md` is the authoritative
navigation and feature tree; this document records module boundaries, reference
rules, and the mapping surface that plans and ADRs may rely on.

## Runtime boundaries

| Layer | Location | Owns | Must not own |
|---|---|---|---|
| Domain/core | `crates/lumilio-core` | launcher state, version metadata, downloads, authentication, instance operations, and independently testable policies | GPUI or `gpui-component` types, view state, blocking UI concerns |
| UI | `crates/lumilio-ui` | Home, Library, Discover, Activity, Accounts, Settings, and Instance pages; GPUI state and interaction | domain rules that belong in core, blocking I/O on the UI thread |
| Application | `crates/lumilio-app` | process startup, configuration, logging, and composition of core and UI | reusable domain behavior |

Dependency direction is `lumilio-app → lumilio-ui → lumilio-core`; core stays
independent of UI so the domain remains testable if the UI framework changes.
Async work belongs in core services or background tasks and crosses into UI via
the GPUI spawn/channel boundary.

## ARCH mapping

| ARCH node | UI responsibility | Core contract to stabilize |
|---|---|---|
| Home | Continue, Recent, Needs Attention; hero reflecting Home state (ADR 0004) and the launch moment | resumable activity and attention summaries; `LaunchSession` launch progress (ADR 0005) |
| Library | All Instances, Favorites, User Collections | instance catalog and collection queries |
| Discover | Modpacks, Mods, Resource Packs, Shaders | discovery metadata and install intents |
| Activity | Downloads, Installs, Updates, Repairs | durable task state and progress events |
| Accounts | Offline and Microsoft accounts; choose the one used to play (ADR 0012) | account store and selection (`LauncherSettings`), Microsoft sign-in |
| Settings | Launcher-wide preferences and defaults inherited by instances (ADR 0012) | `LauncherSettings` validation and commit |
| Instance | Overview, Content, Worlds, History, Diagnostics, Settings | instance lifecycle, content operations, diagnostics, and settings |

Every new page, navigation item, or feature must map to one of these nodes or
first obtain an ADR that changes the scope. Features outside `ARCH.md` are not
implicitly in scope.

## Reference mapping protocol

`3rd-party/HMCL` is read-only. Before consulting a file, add an explicit row
to the table below from the relevant plan or architecture update, and read the
named file. Per [ADR 0011](../.agents/decisions/0011-allow-hmcl-derivation-agpl.md)
code may be adapted into `crates/` (the project ships under AGPL), keeping the
source path and HMCL's GPL-3.0 notice in a comment and respecting any file with
a different license.

| Mapping ID | LumilioCL destination | Authorized reference file | Behavior to extract |
|---|---|---|---|
| CORE-ENV-001 | `lumilio-core::environment` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/CompatibilityRule.java` | Ordered allow/deny defaults plus OS and feature matching |
| CORE-ENV-002 | `lumilio-core::environment` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/OSRestriction.java` | Platform name, architecture, and version matching behavior |
| CORE-ART-001 | `lumilio-core::artifact` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/Artifact.java` | Coordinate parsing and repository-relative artifact paths |
| CORE-REL-001 | `lumilio-core::release` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/Library.java` | Library applicability, native selection, and download lookup |
| CORE-REL-002 | `lumilio-core::release` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/GameVersion.java` | Version identity detection from game archives and fallback order |
| CORE-REL-006 | `lumilio-core::release` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/GameInstanceManifest.java` | Version JSON fields, inheritance markers, patches, and immutable updates |
| CORE-REL-007 | `lumilio-core::release` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/Arguments.java` | Modern game/JVM argument groups and default JVM templates |
| CORE-REL-008 | `lumilio-core::release` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/DownloadInfo.java` | Common downloadable object metadata |
| CORE-REL-009 | `lumilio-core::release` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/LibraryDownloadInfo.java` | Library artifact download metadata and path fallback |
| CORE-REL-010 | `lumilio-core::release` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/LibrariesDownloadInfo.java` | Primary and classified library downloads |
| CORE-REL-011 | `lumilio-core::release` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/AssetIndexInfo.java` | Asset index identity and download metadata |
| CORE-REL-012 | `lumilio-core::release` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/LoggingInfo.java` | Client logging configuration metadata |
| CORE-REL-013 | `lumilio-core::release` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/GameJavaVersion.java` | Java component name and required major version |
| CORE-REL-003 | `lumilio-core::release` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/Argument.java` | Argument expansion inputs and multi-token outputs |
| CORE-REL-004 | `lumilio-core::release` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/RuledArgument.java` | Rule-gated argument behavior |
| CORE-REL-005 | `lumilio-core::release` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/StringArgument.java` | Unconditional argument behavior |
| CORE-LAUNCH-001 | `lumilio-core::launch` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/GameInstanceLibraryBuilder.java` | Classpath ordering, deduplication, and native resolution |
| CORE-LAUNCH-002 | `lumilio-core::launch` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/LaunchManifestNormalizer.java` | Legacy/current metadata normalization into launch inputs |
| CORE-ACT-001 | `lumilio-core::activity` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/task/Task.java` | Lifecycle, dependency execution, cancellation, and progress aggregation |
| CORE-ACT-002 | `lumilio-core::activity` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/task/TaskEvent.java` | Observable start/update/succeed/fail event boundaries |
| CORE-ACT-003 | `lumilio-core::activity` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/task/TaskExecutor.java` | Executor completion and cancellation propagation |
| CORE-XFER-001 | `lumilio-core::transfer` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/task/FileDownloadTask.java` | Destination reuse, temporary files, verification, and retry cases |
| CORE-XFER-002 | `lumilio-core::transfer` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/task/FetchTask.java` | HTTP fetch validation and retry boundaries |
| CORE-XFER-003 | `lumilio-core::transfer` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/task/CacheFileTask.java` | Cache hit, checksum validation, and copy fallback |
| CORE-XFER-004 | `lumilio-core::transfer` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/download/DownloadProvider.java` | Source URL transformation contracts |
| CORE-XFER-005 | `lumilio-core::transfer` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/download/AutoDownloadProvider.java` | Source selection and fallback ordering |
| CORE-INSTALL-001 | `lumilio-core::install` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/download/game/GameInstallTask.java` | Installation stage ordering and completion conditions |
| CORE-INSTALL-002 | `lumilio-core::install` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/download/game/GameDownloadTask.java` | Aggregate client/library/asset download composition |
| CORE-INSTALL-003 | `lumilio-core::install` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/download/game/GameInstanceJsonDownloadTask.java` | Manifest fetch, identity validation, and persistence boundary |
| CORE-INSTALL-004 | `lumilio-core::install` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/download/game/GameLibrariesTask.java` | Library selection and batch scheduling behavior |
| CORE-INSTALL-005 | `lumilio-core::install` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/download/game/LibraryDownloadTask.java` | Library destination, integrity, and native archive handling |
| CORE-INSTALL-006 | `lumilio-core::install` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/download/game/GameAssetIndexDownloadTask.java` | Asset-index retrieval, identity, and validation |
| CORE-INSTALL-007 | `lumilio-core::install` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/download/game/GameAssetDownloadTask.java` | Object-address expansion, legacy mappings, and concurrency |
| CORE-REPAIR-001 | `lumilio-core::repair` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/download/game/GameVerificationFixTask.java` | Verification categories and repair task composition |
| CORE-CATALOG-001 | `lumilio-core::catalog` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/download/game/GameVersionList.java` | Version list refresh, unlisted-version merge, newest-first ordering |
| CORE-CATALOG-002 | `lumilio-core::catalog` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/download/game/GameRemoteVersionInfo.java` | Catalog entry fields and validation |
| CORE-ACCT-001 | `lumilio-core::account` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/auth/offline/OfflineAccount.java` | Offline profile identity and launch auth values |
| CORE-JAVA-001 | `lumilio-core::java` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/java/JavaInfo.java` | Java version parsing, `release` file properties, vendor normalization |
| CORE-PROC-001 | `lumilio-core::process` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/launch/DefaultLauncher.java` | Command-line assembly order, memory flags, default JVM flags |
| CORE-DISC-001 | `lumilio-core::discover` | `3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/addon/repository/ModrinthRemoteAddonRepository.java` | Search facets, sort orders, project/version field usage, file selection, install folders, category tag list, search paging |

## Verification boundaries

Core behavior has tests independent of GPUI. UI work uses the existing
`gpui`/`gpui-component` patterns and keeps I/O off the UI thread. Every change
closes with the four commands in `AGENTS.md`; plans record any environment
blocker rather than weakening the boundary.

## App State storage planning

[App State specification](app-state.md) defines the proposed platform roots,
persistence ownership, lifecycle and migration plan. [ADR 0007](../.agents/decisions/0007-platform-app-state.md)
is proposed; ADR 0006 and [current storage behavior](behavior/storage.md)
remain the implemented baseline until the migration work lands.

## User workflow reference mapping

[Workflow guide](workflows/README.md) connects user goals, the local HMCL behavior inventory,
LumilioCL target contracts and verification scenarios. It does not extend `ARCH.md`.
`FLOW-REF-*` entries authorize only the exact files below; no recursive reference reads.
Baseline: `587e92543a8926861f99fe11629d23dd94d090d8` (2026-08-25).
For a new reference read, add a row here first and update the relevant workflow evidence.

| Mapping ID | LumilioCL destination | Authorized reference file | Behavior to extract |
|---|---|---|---|
| FLOW-REF-001 | docs/workflows — 导航、实例与启动 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/RootPage.java` | RootPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-002 | docs/workflows — 导航、实例与启动 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/MainPage.java` | MainPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-003 | docs/workflows — 导航、实例与启动 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/GameListPage.java` | GameListPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-004 | docs/workflows — 导航、实例与启动 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/GameListCell.java` | GameListCell: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-005 | docs/workflows — 导航、实例与启动 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/GameListItem.java` | GameListItem: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-006 | docs/workflows — 导航、实例与启动 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/GameListPopupMenu.java` | GameListPopupMenu: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-007 | docs/workflows — 导航、实例与启动 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/GameInstancePage.java` | GameInstancePage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-008 | docs/workflows — 导航、实例与启动 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/Instances.java` | Instances: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-009 | docs/workflows — 导航、实例与启动 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/directory/GameDirectoryPage.java` | GameDirectoryPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-010 | docs/workflows — 导航、实例与启动 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/directory/GameDirectoryListItem.java` | GameDirectoryListItem: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-011 | docs/workflows — 导航、实例与启动 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/directory/GameDirectoryListItemSkin.java` | GameDirectoryListItemSkin: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-012 | docs/workflows — 导航、实例与启动 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/game/LauncherHelper.java` | LauncherHelper: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-013 | docs/workflows — 导航、实例与启动 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/LogWindow.java` | LogWindow: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-014 | docs/workflows — 导航、实例与启动 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/GameCrashWindow.java` | GameCrashWindow: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-015 | docs/workflows — 安装、导入与导出 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/DownloadPage.java` | DownloadPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-016 | docs/workflows — 安装、导入与导出 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/VersionsPage.java` | VersionsPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-017 | docs/workflows — 安装、导入与导出 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/InstallersPage.java` | InstallersPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-018 | docs/workflows — 安装、导入与导出 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/AbstractInstallersPage.java` | AbstractInstallersPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-019 | docs/workflows — 安装、导入与导出 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/AdditionalInstallersPage.java` | AdditionalInstallersPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-020 | docs/workflows — 安装、导入与导出 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/UpdateInstallerWizardProvider.java` | UpdateInstallerWizardProvider: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-021 | docs/workflows — 安装、导入与导出 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/ModpackInstallWizardProvider.java` | ModpackInstallWizardProvider: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-022 | docs/workflows — 安装、导入与导出 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/ModpackSelectionPage.java` | ModpackSelectionPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-023 | docs/workflows — 安装、导入与导出 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/LocalModpackPage.java` | LocalModpackPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-024 | docs/workflows — 安装、导入与导出 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/RemoteModpackPage.java` | RemoteModpackPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-025 | docs/workflows — 安装、导入与导出 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/download/ModpackPage.java` | ModpackPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-026 | docs/workflows — 安装、导入与导出 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/InstallerListPage.java` | InstallerListPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-027 | docs/workflows — 安装、导入与导出 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/export/ExportWizardProvider.java` | ExportWizardProvider: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-028 | docs/workflows — 安装、导入与导出 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/export/ModpackTypeSelectionPage.java` | ModpackTypeSelectionPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-029 | docs/workflows — 安装、导入与导出 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/export/ModpackInfoPage.java` | ModpackInfoPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-030 | docs/workflows — 安装、导入与导出 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/export/ModpackFileSelectionPage.java` | ModpackFileSelectionPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-031 | docs/workflows — 安装、导入与导出 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/game/ModpackHelper.java` | ModpackHelper: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-032 | docs/workflows — 账户 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/account/AccountListPage.java` | AccountListPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-033 | docs/workflows — 账户 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/account/AccountListPopupMenu.java` | AccountListPopupMenu: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-034 | docs/workflows — 账户 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/account/CreateAccountPane.java` | CreateAccountPane: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-035 | docs/workflows — 账户 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/account/MicrosoftAccountLoginPane.java` | MicrosoftAccountLoginPane: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-036 | docs/workflows — 账户 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/account/AddAuthlibInjectorServerPane.java` | AddAuthlibInjectorServerPane: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-037 | docs/workflows — 账户 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/account/AccountListItem.java` | AccountListItem: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-038 | docs/workflows — 账户 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/account/AccountListItemSkin.java` | AccountListItemSkin: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-039 | docs/workflows — 账户 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/account/OfflineAccountSkinPane.java` | OfflineAccountSkinPane: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-040 | docs/workflows — 内容与世界 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/DownloadListPage.java` | DownloadListPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-041 | docs/workflows — 内容与世界 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/DownloadPage.java` | DownloadPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-042 | docs/workflows — 内容与世界 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/HMCLLocalizedDownloadListPage.java` | HMCLLocalizedDownloadListPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-043 | docs/workflows — 内容与世界 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/ModListPage.java` | ModListPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-044 | docs/workflows — 内容与世界 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/ModListPageSkin.java` | ModListPageSkin: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-045 | docs/workflows — 内容与世界 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/ResourcePackListPage.java` | ResourcePackListPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-046 | docs/workflows — 内容与世界 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/AddonUpdatesPage.java` | AddonUpdatesPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-047 | docs/workflows — 内容与世界 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/AddonCheckUpdatesTask.java` | AddonCheckUpdatesTask: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-048 | docs/workflows — 内容与世界 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/WorldListPage.java` | WorldListPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-049 | docs/workflows — 内容与世界 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/WorldManagePage.java` | WorldManagePage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-050 | docs/workflows — 内容与世界 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/WorldInfoPage.java` | WorldInfoPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-051 | docs/workflows — 内容与世界 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/WorldBackupsPage.java` | WorldBackupsPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-052 | docs/workflows — 内容与世界 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/WorldBackupTask.java` | WorldBackupTask: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-053 | docs/workflows — 内容与世界 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/WorldExportPage.java` | WorldExportPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-054 | docs/workflows — 内容与世界 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/WorldExportPageSkin.java` | WorldExportPageSkin: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-055 | docs/workflows — 内容与世界 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/WorldManageUIUtils.java` | WorldManageUIUtils: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-056 | docs/workflows — 内容与世界 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/DataPackListPage.java` | DataPackListPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-057 | docs/workflows — 内容与世界 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/DataPackListPageSkin.java` | DataPackListPageSkin: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-058 | docs/workflows — 设置、Java 与应用维护 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/LauncherSettingsPage.java` | LauncherSettingsPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-059 | docs/workflows — 设置、Java 与应用维护 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/SettingsPage.java` | SettingsPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-060 | docs/workflows — 设置、Java 与应用维护 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/DownloadSettingsPage.java` | DownloadSettingsPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-061 | docs/workflows — 设置、Java 与应用维护 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/PersonalizationPage.java` | PersonalizationPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-062 | docs/workflows — 设置、Java 与应用维护 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/ThemePackManagementPage.java` | ThemePackManagementPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-063 | docs/workflows — 设置、Java 与应用维护 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/JavaManagementPage.java` | JavaManagementPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-064 | docs/workflows — 设置、Java 与应用维护 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/JavaInstallPage.java` | JavaInstallPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-065 | docs/workflows — 设置、Java 与应用维护 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/JavaDownloadDialog.java` | JavaDownloadDialog: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-066 | docs/workflows — 设置、Java 与应用维护 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/JavaRestorePage.java` | JavaRestorePage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-067 | docs/workflows — 设置、Java 与应用维护 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/game/GameSettingsPage.java` | GameSettingsPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-068 | docs/workflows — 设置、Java 与应用维护 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/game/PresetManagementPane.java` | PresetManagementPane: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-069 | docs/workflows — 设置、Java 与应用维护 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/HelpPage.java` | HelpPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-070 | docs/workflows — 设置、Java 与应用维护 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/FeedbackPage.java` | FeedbackPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-071 | docs/workflows — 设置、Java 与应用维护 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/main/AboutPage.java` | AboutPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-072 | docs/workflows — 设置、Java 与应用维护 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/UpgradeDialog.java` | UpgradeDialog: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-073 | docs/workflows — 设置、Java 与应用维护 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/construct/TaskExecutorDialogPane.java` | TaskExecutorDialogPane: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-074 | docs/workflows — 设置、Java 与应用维护 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/wizard/TaskExecutorDialogWizardDisplayer.java` | TaskExecutorDialogWizardDisplayer: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-075 | docs/workflows — 范围外候选核对 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/terracotta/TerracottaPage.java` | TerracottaPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-076 | docs/workflows — 范围外候选核对 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/terracotta/TerracottaControllerPage.java` | TerracottaControllerPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-077 | docs/workflows — 范围外候选核对 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/instances/SchematicsPage.java` | SchematicsPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-078 | docs/workflows — 范围外候选核对 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/ui/nbt/NBTEditorPage.java` | NBTEditorPage: user entry, conditions, branches, file effects, cancellation/failure boundaries |
| FLOW-REF-079 | docs/workflows — 目录登记移除、选中目标和文件边界 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/setting/GameDirectoryManager.java` | 目录登记移除、选中目标和文件边界 |
| FLOW-REF-080 | docs/workflows — 实例删除、复制、重命名与草稿发布行为 | `3rd-party/HMCL/HMCL/src/main/java/org/jackhuang/hmcl/game/HMCLGameRepository.java` | 实例删除、复制、重命名与草稿发布行为 |
