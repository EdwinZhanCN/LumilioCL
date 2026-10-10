// A streaming mirror of LumilioCL's GitHub release downloads, for networks
// where github.com and its asset hosts are slow or unreachable.
//
// The approach follows gh-proxy (https://github.com/hunshcn/gh-proxy, MIT):
// fetch with `redirect: "manual"`, follow GitHub's redirects to its asset
// host inside the Worker, and stream the body straight back. Unlike
// gh-proxy it is not an open proxy: only this repository's release
// downloads are served, and redirects may only lead to GitHub's own hosts.
// Nothing is cached at the edge and nothing is verified here; the launcher
// and the checksums file verify what arrives.

export const REPO = "EdwinZhanCN/LumilioCL";

/** Hosts a GitHub release download may redirect through. */
export const REDIRECT_HOSTS = new Set([
  "github.com",
  "objects.githubusercontent.com",
  "release-assets.githubusercontent.com",
]);

export const MAX_HOPS = 5;

// Release tags are SemVer with a leading v (PACKAGING.md §5).
const TAG = /^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;
// One path segment of letters, digits, dots, dashes and underscores.
const FILE = /^[0-9A-Za-z][0-9A-Za-z._-]*$/;

// Response headers worth passing on; everything else from GitHub stays there.
const PASS_HEADERS = [
  "accept-ranges",
  "content-disposition",
  "content-length",
  "content-range",
  "content-type",
  "etag",
  "last-modified",
];

// Request headers worth passing upstream.
const FORWARD_HEADERS = ["range", "if-none-match", "if-modified-since", "if-range"];

export type Upstream = { url: string; /** browser cache lifetime, seconds */ maxAge: number };

/** A file segment, judged by what it decodes to; a malformed escape is no file. */
function isFile(segment: string): boolean {
  try {
    return FILE.test(decodeURIComponent(segment));
  } catch {
    return false;
  }
}

/** Maps a mirror path to the GitHub address it stands for, or null. */
export function resolve(pathname: string): Upstream | null {
  const parts = pathname.split("/").slice(1);
  // /releases/download/<tag>/<file>: a fixed release, never changes.
  if (parts.length === 4 && parts[0] === "releases" && parts[1] === "download") {
    const [, , tag, file] = parts;
    if (!TAG.test(tag) || !isFile(file)) return null;
    return { url: `https://github.com/${REPO}/releases/download/${tag}/${file}`, maxAge: 86400 };
  }
  // /releases/latest/download/<file>: whatever the newest published release
  // has under that name, so it may change at any moment.
  if (parts.length === 4 && parts[0] === "releases" && parts[1] === "latest" && parts[2] === "download") {
    const file = parts[3];
    if (!isFile(file)) return null;
    return { url: `https://github.com/${REPO}/releases/latest/download/${file}`, maxAge: 60 };
  }
  return null;
}

/** Proxies one request. `fetcher` is the global fetch, injectable for tests. */
export async function proxy(request: Request, fetcher: typeof fetch = fetch): Promise<Response> {
  if (request.method !== "GET" && request.method !== "HEAD")
    return new Response("Method not allowed", { status: 405, headers: { allow: "GET, HEAD" } });

  const upstream = resolve(new URL(request.url).pathname);
  if (!upstream) return new Response("Not found", { status: 404 });

  const headers = new Headers({ "user-agent": "lumilio-mirror" });
  for (const name of FORWARD_HEADERS) {
    const value = request.headers.get(name);
    if (value) headers.set(name, value);
  }

  let target = upstream.url;
  for (let hop = 0; ; hop++) {
    const response = await fetcher(target, { method: request.method, headers, redirect: "manual" });
    if (response.status < 300 || response.status >= 400 || response.status === 304) return relay(response, upstream);

    const location = response.headers.get("location");
    if (!location || hop >= MAX_HOPS) return new Response("Bad gateway", { status: 502 });
    const next = new URL(location, target);
    if (next.protocol !== "https:" || !REDIRECT_HOSTS.has(next.hostname))
      return new Response("Bad gateway", { status: 502 });
    target = next.toString();
  }
}

function relay(response: Response, upstream: Upstream): Response {
  const headers = new Headers();
  for (const name of PASS_HEADERS) {
    const value = response.headers.get(name);
    if (value) headers.set(name, value);
  }
  headers.set("access-control-allow-origin", "*");
  headers.set("access-control-expose-headers", "content-length, content-range, etag");
  if (response.ok) headers.set("cache-control", `public, max-age=${upstream.maxAge}`);
  else headers.set("cache-control", "no-store");
  // The body streams through as it arrives; a 206 stays a 206.
  return new Response(response.body, { status: response.status, headers });
}
