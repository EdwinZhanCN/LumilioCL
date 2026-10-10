// Measured, not estimated. To re-measure: quit both apps, launch each one
// three times, and read `footprint -p <pid>` 30 s after launch, summing the
// app's process and the system WebKit helpers that appear with it; record
// the median. (Modrinth App peaks far higher while starting, then settles;
// the settled value is the fair one.) Sizes are `du -sh` of the installed
// .app. Update the versions and date with the numbers.
export const BENCHMARK = {
  date: "2026 年 10 月 9 日",
  machine: "Apple M2 Pro、16 GB 内存的 Mac",
  os: "macOS 27.2",
  runs: 3,
  us: { version: "0.1.0", memoryMb: 106, processes: 1, sizeMb: 67 },
  them: { name: "Modrinth App", version: "0.21.9", memoryMb: 404, processes: 5, sizeMb: 88 },
} as const;
