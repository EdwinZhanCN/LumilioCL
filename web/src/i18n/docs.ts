export type DocsLanguage = "zh-CN" | "en";

const en = {
  roadmap: {
    label: "Primary plans",
    target: "Target version", noVersion: "Not assigned", source: "Read the primary plan",
    empty: "No primary plans are published.",
    status: { proposed: "Proposed", in_progress: "In progress", blocked: "Blocked", completed: "Completed", cancelled: "Cancelled" },
    plans: {
      "release-0-1-0": { name: "First release", lede: "Packages for three platforms with automatic updates." },
      appearance: { name: "Themes and appearance", lede: "More themes, interface fonts, and text sizes." },
      "discover-translation": { name: "Project translation", lede: "Translate project descriptions and release notes into the interface language." },
      "wasm-plugin-registry": { name: "Community plugins", lede: "WASM plugins and a plugin directory with review controls." },
    } as Record<string, { name: string; lede: string }>,
  },
  changelog: {
    published: "Published", prerelease: "Pre-release", stable: "Release", source: "Read on GitHub",
    noNotes: "This release has no notes. Read the release on GitHub for its files.",
    empty: "No published releases are available.",
    unavailable: "Release notes are not available here. Read the releases on GitHub.",
    all: "All GitHub releases",
  },
};

const zhCN: typeof en = {
  roadmap: {
    label: "主计划",
    target: "目标版本", noVersion: "尚未指定", source: "查看主计划",
    empty: "目前没有公开的主计划。",
    status: { proposed: "提议中", in_progress: "进行中", blocked: "受阻", completed: "已完成", cancelled: "已取消" },
    plans: {},
  },
  changelog: {
    published: "发布时间", prerelease: "预发布", stable: "正式发布", source: "在 GitHub 查看",
    noNotes: "该版本未提供发布说明。可在 GitHub 查看发布文件。",
    empty: "目前没有已发布的版本。", unavailable: "暂时无法在此读取发布说明。请前往 GitHub Releases 查看。",
    all: "查看全部 GitHub Releases",
  },
};

export function docsCatalog(language: string) {
  return language.startsWith("en") ? en : zhCN;
}
