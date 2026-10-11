import { afterEach, describe, expect, it, vi } from "vitest";
import { loadReleaseNotes, publishedReleaseNotes, renderReleaseNote } from "./release-notes";
import { RELEASES_URL } from "./links";

function release(id: number, overrides = {}) {
  return {
    id, tag_name: `v1.0.${id}`, name: null, html_url: `${RELEASES_URL}/tag/v1.0.${id}`,
    published_at: "2026-10-10T10:00:00Z", draft: false, prerelease: false, body: "## Changes\n\nA **new** feature.",
    ...overrides,
  };
}

afterEach(() => vi.unstubAllEnvs());

describe("published release notes", () => {
  it("excludes drafts and unpublished entries, retains pre-releases and orders by publication", () => {
    const notes = publishedReleaseNotes([
      release(1, { draft: true, body: "private draft" }),
      release(2, { published_at: null }),
      release(3, { published_at: "2026-10-09T10:00:00Z" }),
      release(4, { prerelease: true }),
    ]);
    expect(notes.map((note) => note.id)).toEqual([4, 3]);
    expect(notes[0].prerelease).toBe(true);
    expect(JSON.stringify(notes)).not.toContain("private draft");
    expect(notes[0].html).toContain("<strong>new</strong>");
  });

  it("renders Markdown but removes scripts, event handlers and unsafe links", () => {
    const html = renderReleaseNote(
      "# Changes\n\n<script>alert(1)</script><img src=\"https://example.com/image.png\" onerror=\"alert(2)\">\n\n[bad](javascript:alert) [source](README.md) [section](#changes)",
      `${RELEASES_URL}/tag/v1.0.0`, "v1.0.0",
    );
    expect(html).not.toMatch(/<script|onerror|javascript:|alert\(1\)/i);
    expect(html).toContain("<h3>Changes</h3>");
    expect(html).toContain("https://example.com/image.png");
    expect(html).toContain("/blob/v1.0.0/README.md");
    expect(html).toContain("/releases/tag/v1.0.0#changes");
  });

  it("loads every API page without exposing an authenticated draft", async () => {
    vi.stubEnv("LUMILIO_RELEASE_NOTES_FIXTURE", "");
    const fetcher = vi.fn<typeof fetch>()
      .mockResolvedValueOnce(Response.json(Array.from({ length: 100 }, (_, id) => release(id, { draft: true }))))
      .mockResolvedValueOnce(Response.json([release(101)]));
    const notes = await loadReleaseNotes(fetcher);
    expect(fetcher).toHaveBeenCalledTimes(2);
    expect(fetcher.mock.calls[1][0]).toContain("page=2");
    expect(notes.available).toBe(true);
    expect(notes.releases.map((note) => note.id)).toEqual([101]);
  });

  it("distinguishes an empty release history from an API failure", async () => {
    vi.stubEnv("LUMILIO_RELEASE_NOTES_FIXTURE", "");
    expect(await loadReleaseNotes(vi.fn<typeof fetch>().mockResolvedValue(Response.json([]))))
      .toEqual({ available: true, releases: [] });
    expect(await loadReleaseNotes(vi.fn<typeof fetch>().mockResolvedValue(new Response("", { status: 403 }))))
      .toEqual({ available: false, releases: [] });
    expect(await loadReleaseNotes(vi.fn<typeof fetch>().mockRejectedValue(new Error("offline"))))
      .toEqual({ available: false, releases: [] });
  });
});
