# Installation and repair behavior

This contract describes LumilioCL's installation transaction in its own domain
terms. Mapping IDs `CORE-INSTALL-*` and `CORE-REPAIR-*` identify the read-only
behavior references; the Rust model is independent of their task hierarchy.

## Installation plan

An installation plan is created only from a resolved release and its launch
plan. Their release identities must agree. The plan contains the encoded
release manifest, all host-applicable client, library, native, logging, and
asset-catalog transfers, the native extraction policies, and the destination
layout. A downloadable item without a usable source is rejected while planning
instead of becoming a late launch failure.

Paths originating in protocol data are relative paths. Empty paths, absolute
paths, platform prefixes, current-directory components, parent traversal, and
backslash aliases are rejected before filesystem access. Transfer identifiers
are deterministic and unique within a plan.

## Asset catalog

An asset catalog maps logical resource names to content objects. Every object
has a 40-digit hexadecimal SHA-1 identity and a declared byte length. Its
canonical location is `objects/<first two hash digits>/<full hash>`, and its
official source uses the same two-segment suffix. Multiple logical names may
refer to one object; the object is transferred and verified once.

The catalog download is refreshed during a new install. It is parsed before
any object transfer is scheduled. Invalid JSON, invalid hashes, or unsafe
logical names stop the transaction. When the catalog requests legacy virtual
or resource layouts, verified objects are published to those views through
temporary siblings; the content-addressed object remains the source of truth.

## Transaction stages

The executor observes these completion boundaries:

1. Transfer base artifacts and the asset catalog with bounded concurrency.
2. Decode the catalog, expand unique content objects, and transfer them.
3. Publish requested legacy asset views.
4. Extract all native archives into a private staging directory.
5. Atomically replace the native directory and then atomically publish the
   encoded release manifest.

A failed or cancelled stage does not start later stages. Successful downloads
remain reusable, but an older manifest and native directory stay intact until
their replacements are ready. The report retains the outcome of every transfer
and the number of published asset/native files.

## Archive safety

Native entries are accepted only as strict relative paths made of normal path
components. Absolute paths, parent traversal, current-directory components,
backslash aliases, NUL bytes, symbolic links, and other special file types are
rejected. Exclusion prefixes are matched against normalized archive names
before extraction. Declared entry count, per-file size, and aggregate expanded
size are bounded.

No archive entry writes directly to the live native directory. Any validation,
read, write, cancellation, or publication failure removes the staging output
and preserves the previous live directory.

## Verification and repair

Verification compares the installation plan and decoded asset catalog with the
filesystem. A required artifact is classified as missing, wrong-sized,
wrong-checksummed, malformed catalog, or valid. Size and checksum are applied
only when metadata supplies them; a path must always be a regular file.

A repair plan contains exactly the transfer requests for invalid artifacts.
Valid siblings are not scheduled again. Catalog repair precedes catalog decode,
so object repair is produced only from a valid catalog. Repair uses the same
transfer and publication rules as installation.

## Publication 写入协调

Installer::execute 按排序后的资源键持有 natives 目录与 release manifest 的进程内锁，直到 execute 结束；同一发布目标的安装不能交叉发布，等待可取消。各下载另持文件目标键。此规则尚不等于游戏运行期间的只读资源引用保护，也不自动协调直接调用低层 NativePublisher 的外部使用者。
