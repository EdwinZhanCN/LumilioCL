import { marked } from "marked";
import sanitizeHtml from "sanitize-html";
import { REPO, RELEASES_URL } from "./links";

export type ReleaseNote = {
  id: number;
  tag: string;
  name: string;
  url: string;
  publishedAt: string;
  prerelease: boolean;
  html: string;
};

export type ReleaseNotes = { available: boolean; releases: ReleaseNote[] };

type GitHubRelease = {
  id: number;
  tag_name: string;
  name: string | null;
  html_url: string;
  published_at: string | null;
  draft: boolean;
  prerelease: boolean;
  body: string | null;
};

export function renderReleaseNote(body: string, source: string, tag: string): string {
  const rendered = marked.parse(body, { async: false, gfm: true });
  const linkBase = `https://github.com/${REPO}/blob/${encodeURIComponent(tag)}/`;
  const imageBase = `https://raw.githubusercontent.com/${REPO}/${encodeURIComponent(tag)}/`;
  const resolve = (value: string | undefined, base: string) => {
    try { return value ? new URL(value, base).href : ""; }
    catch { return ""; }
  };
  return sanitizeHtml(rendered, {
    allowedTags: [...sanitizeHtml.defaults.allowedTags, "img", "del"],
    allowedAttributes: {
      ...sanitizeHtml.defaults.allowedAttributes,
      a: ["href", "title", "rel"],
      img: ["src", "alt", "title", "loading"],
      code: ["class"],
    },
    allowedSchemes: ["https", "http", "mailto"],
    allowProtocolRelative: false,
    transformTags: {
      a: (_tag, attrs) => ({
        tagName: "a",
        attribs: {
          ...attrs,
          href: resolve(attrs.href, attrs.href?.startsWith("#") ? source : linkBase),
          rel: "nofollow noopener noreferrer",
        },
      }),
      img: (_tag, attrs) => ({
        tagName: "img", attribs: { ...attrs, src: resolve(attrs.src, imageBase), loading: "lazy" },
      }),
      h1: sanitizeHtml.simpleTransform("h3", {}),
      h2: sanitizeHtml.simpleTransform("h3", {}),
      h3: sanitizeHtml.simpleTransform("h4", {}),
      h4: sanitizeHtml.simpleTransform("h5", {}),
      h5: sanitizeHtml.simpleTransform("h6", {}),
    },
  });
}

export function publishedReleaseNotes(input: GitHubRelease[]): ReleaseNote[] {
  return input.filter((release) => !release.draft && release.published_at !== null).map((release) => {
    if (
      !Number.isInteger(release.id) || !release.tag_name ||
      !Number.isFinite(Date.parse(release.published_at!)) ||
      !release.html_url.startsWith(`${RELEASES_URL}/tag/`)
    ) throw new Error("Invalid published release data");
    return {
      id: release.id,
      tag: release.tag_name,
      name: release.name || release.tag_name,
      url: release.html_url,
      publishedAt: release.published_at!,
      prerelease: release.prerelease,
      html: renderReleaseNote(release.body ?? "", release.html_url, release.tag_name),
    };
  }).sort((a, b) => Date.parse(b.publishedAt) - Date.parse(a.publishedAt));
}

// Fetch only at build time. A release event rebuilds the static site.
export async function loadReleaseNotes(fetcher: typeof fetch = fetch): Promise<ReleaseNotes> {
  try {
    const fixture = process.env.LUMILIO_RELEASE_NOTES_FIXTURE;
    if (fixture) {
      const { readFile } = await import("node:fs/promises");
      return { available: true, releases: publishedReleaseNotes(JSON.parse(await readFile(fixture, "utf8"))) };
    }
    const headers: Record<string, string> = {
      accept: "application/vnd.github+json",
      "user-agent": "lumilio-web-build",
      "X-GitHub-Api-Version": "2026-03-10",
    };
    if (process.env.GITHUB_TOKEN) headers.authorization = `Bearer ${process.env.GITHUB_TOKEN}`;
    const releases: GitHubRelease[] = [];
    for (let page = 1; ; page++) {
      const response = await fetcher(`https://api.github.com/repos/${REPO}/releases?per_page=100&page=${page}`, {
        headers, signal: AbortSignal.timeout(10_000),
      });
      if (!response.ok) throw new Error(`Release API returned ${response.status}`);
      const batch: unknown = await response.json();
      if (!Array.isArray(batch)) throw new Error("Invalid release list");
      releases.push(...batch);
      if (batch.length < 100) break;
    }
    const notes = publishedReleaseNotes(releases);
    return { available: true, releases: [...new Map(notes.map((note) => [note.id, note])).values()] };
  } catch {
    return { available: false, releases: [] };
  }
}

let cached: Promise<ReleaseNotes> | undefined;
export function releaseNotes(): Promise<ReleaseNotes> {
  return cached ??= loadReleaseNotes();
}
