// The latest published release, read once at build time. Published releases
// only: GitHub's `latest` skips drafts and pre-releases. The web workflow
// rebuilds on `release: published`, which keeps the page current.

import { MIRROR, REPO } from "./links";

export type PlatformId = "macos" | "windows" | "linux";
export type FileKind = "dmg" | "setup" | "portable" | "deb" | "tar";

export type ReleaseFile = {
  kind: FileKind;
  name: string;
  size: number;
  /** Through the Worker at launcher.lumilio.org. */
  mirror: string;
  /** GitHub's own address, the fallback. */
  github: string;
};

export type Release = {
  tag: string;
  version: string;
  platforms: Record<PlatformId, ReleaseFile[]>;
  checksums: { mirror: string; github: string } | null;
};

// File names follow PACKAGING.md §5: LumilioCL-<version>-<os>-<arch>…
const FILES: { kind: FileKind; platform: PlatformId; pattern: RegExp }[] = [
  { kind: "dmg", platform: "macos", pattern: /-macos-arm64\.dmg$/ },
  { kind: "setup", platform: "windows", pattern: /-windows-x64-setup\.exe$/ },
  { kind: "portable", platform: "windows", pattern: /-windows-x64-portable\.zip$/ },
  { kind: "deb", platform: "linux", pattern: /-linux-x64\.deb$/ },
  { kind: "tar", platform: "linux", pattern: /-linux-x64\.tar\.gz$/ },
];

type GithubAsset = { name: string; size: number; browser_download_url: string };

let cached: Promise<Release | null> | undefined;

export function latestRelease(): Promise<Release | null> {
  cached ??= load();
  return cached;
}

async function fetchLatest(): Promise<{ tag_name: string; assets: GithubAsset[] } | null> {
  // A saved API response, to look at the download panel before a release exists.
  const fixture = process.env.LUMILIO_RELEASE_FIXTURE;
  if (fixture) {
    const { readFile } = await import("node:fs/promises");
    return JSON.parse(await readFile(fixture, "utf8"));
  }
  const headers: Record<string, string> = {
    accept: "application/vnd.github+json",
    "user-agent": "lumilio-web-build",
  };
  if (process.env.GITHUB_TOKEN) headers.authorization = `Bearer ${process.env.GITHUB_TOKEN}`;
  const response = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, { headers });
  return response.ok ? response.json() : null;
}

async function load(): Promise<Release | null> {
  try {
    const body = await fetchLatest();
    if (!body) return null;
    const mirror = (name: string) => `${MIRROR}/releases/download/${body.tag_name}/${encodeURIComponent(name)}`;
    const platforms: Release["platforms"] = { macos: [], windows: [], linux: [] };
    for (const { kind, platform, pattern } of FILES) {
      const asset = body.assets.find((a) => pattern.test(a.name));
      if (asset)
        platforms[platform].push({
          kind,
          name: asset.name,
          size: asset.size,
          mirror: mirror(asset.name),
          github: asset.browser_download_url,
        });
    }
    const sums = body.assets.find((a) => a.name === "SHA256SUMS.txt");
    return {
      tag: body.tag_name,
      version: body.tag_name.replace(/^v/, ""),
      platforms,
      checksums: sums ? { mirror: mirror(sums.name), github: sums.browser_download_url } : null,
    };
  } catch {
    return null;
  }
}
