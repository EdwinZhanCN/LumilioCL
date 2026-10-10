import { describe, expect, it, vi } from "vitest";
import { MAX_HOPS, proxy, resolve } from "./proxy";

const MIRROR = "https://launcher.lumilio.org";
const ASSET = "https://release-assets.githubusercontent.com/github-production-release-asset/1?sig=abc";

const redirect = (location: string) => new Response(null, { status: 302, headers: { location } });

describe("resolve", () => {
  it("maps a tagged release file to this repository only", () => {
    expect(resolve("/releases/download/v0.1.0/LumilioCL-0.1.0-macos-arm64.dmg")).toEqual({
      url: "https://github.com/EdwinZhanCN/LumilioCL/releases/download/v0.1.0/LumilioCL-0.1.0-macos-arm64.dmg",
      maxAge: 86400,
    });
    expect(resolve("/releases/download/v0.2.0-beta.1/SHA256SUMS.txt")?.url).toContain("/v0.2.0-beta.1/");
  });

  it("gives the moving latest alias a short lifetime", () => {
    expect(resolve("/releases/latest/download/latest.json")?.maxAge).toBe(60);
  });

  it("refuses anything that could reach another repository or path", () => {
    for (const path of [
      "/releases/download/v0.1.0/../../other/repo",
      "/releases/download/v0.1.0/a/b",
      "/releases/download/main/file.zip",
      "/releases/download/v0.1.0/%2e%2e",
      "/releases/download/v0.1.0/",
      "/EdwinZhanCN/LumilioCL/releases/download/v0.1.0/x.dmg",
      "/releases/latest/download/a%2Fb",
      "/releases/download/v0.1.0/%E0%A4%A",
      "/releases",
    ])
      expect(resolve(path), path).toBeNull();
  });
});

describe("proxy", () => {
  it("follows GitHub's redirect inside the Worker and streams the asset back", async () => {
    const fetcher = vi.fn<typeof fetch>(async (input) =>
      String(input).startsWith("https://github.com/")
        ? redirect(ASSET)
        : new Response("dmg-bytes", { status: 200, headers: { "content-type": "application/octet-stream", "set-cookie": "x=1" } }),
    );
    const response = await proxy(new Request(`${MIRROR}/releases/download/v0.1.0/LumilioCL-0.1.0-macos-arm64.dmg`), fetcher);
    expect(response.status).toBe(200);
    expect(await response.text()).toBe("dmg-bytes");
    expect(response.headers.get("content-type")).toBe("application/octet-stream");
    expect(response.headers.get("set-cookie")).toBeNull();
    expect(response.headers.get("cache-control")).toBe("public, max-age=86400");
    expect(fetcher).toHaveBeenCalledTimes(2);
    expect(String(fetcher.mock.calls[1][0])).toBe(ASSET);
  });

  it("passes a range request through and keeps the 206", async () => {
    const fetcher = vi.fn<typeof fetch>(async (input, init) => {
      if (String(input).startsWith("https://github.com/")) return redirect(ASSET);
      expect(new Headers(init?.headers).get("range")).toBe("bytes=100-199");
      return new Response("x".repeat(100), { status: 206, headers: { "content-range": "bytes 100-199/1000" } });
    });
    const response = await proxy(
      new Request(`${MIRROR}/releases/download/v0.1.0/a.zip`, { headers: { range: "bytes=100-199" } }),
      fetcher,
    );
    expect(response.status).toBe(206);
    expect(response.headers.get("content-range")).toBe("bytes 100-199/1000");
  });

  it("refuses a redirect to a host that is not GitHub's", async () => {
    const fetcher = vi.fn<typeof fetch>(async () => redirect("https://evil.example/payload"));
    const response = await proxy(new Request(`${MIRROR}/releases/download/v0.1.0/a.zip`), fetcher);
    expect(response.status).toBe(502);
    expect(fetcher).toHaveBeenCalledTimes(1);
  });

  it("stops after the hop limit", async () => {
    const fetcher = vi.fn<typeof fetch>(async () => redirect("https://github.com/loop"));
    const response = await proxy(new Request(`${MIRROR}/releases/download/v0.1.0/a.zip`), fetcher);
    expect(response.status).toBe(502);
    expect(fetcher).toHaveBeenCalledTimes(MAX_HOPS + 1);
  });

  it("only serves GET and HEAD", async () => {
    const fetcher = vi.fn<typeof fetch>();
    const response = await proxy(new Request(`${MIRROR}/releases/download/v0.1.0/a.zip`, { method: "POST" }), fetcher);
    expect(response.status).toBe(405);
    expect(response.headers.get("allow")).toBe("GET, HEAD");
    expect(fetcher).not.toHaveBeenCalled();
  });

  it("does not cache GitHub's not-found", async () => {
    const fetcher = vi.fn<typeof fetch>(async () => new Response("Not Found", { status: 404 }));
    const response = await proxy(new Request(`${MIRROR}/releases/download/v9.9.9/a.zip`), fetcher);
    expect(response.status).toBe(404);
    expect(response.headers.get("cache-control")).toBe("no-store");
  });
});
