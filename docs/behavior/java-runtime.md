# Java runtime download behavior (ADR 0014)

Explicit action only: the problem button "安装 Java" on a game that has no suitable Java, and Settings › Java "下载推荐的 Java". Nothing is fetched automatically.

- The index (`launchermeta.mojang.com/.../java-runtime/.../all.json`) lists runtimes per system. The needed Java major picks the newest component of exactly that major; with no major, Java 21 if offered, else the newest. No downloadable runtime for the system (or the major) is an error that says so.
- The component's file list is read before anything is written. A path that is not a plain relative path, a file with no raw download or one from a host other than Mojang's, or a link that would leave the runtime, refuses the whole list.
- Files are downloaded through the normal transfer engine into `runtimes/.<component>.installing` (size and SHA-1 checked), then executable bits and links are made, then one rename publishes `runtimes/<component>`. A failed or cancelled install removes the staging folder and leaves nothing.
- A macOS runtime's `jre.bundle/` prefix is dropped so the result is `<component>/Contents/Home/…`, which Java discovery already finds.
- An already installed component is returned as it is.
- It is an Activity task (byte progress, cancel, retry).
