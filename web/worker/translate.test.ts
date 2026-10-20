import { DatabaseSync } from "node:sqlite";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { type Ai, type Database, DAILY_NEURONS, MAX_IDS, MODEL, readIds, resetSchema, translate } from "./translate";

const SITE = "https://launcher.lumilio.org";

/** A D1 binding over an in-memory SQLite database. */
function d1(): Database & { sqlite: DatabaseSync } {
  const sqlite = new DatabaseSync(":memory:");
  const statement = (query: string, values: unknown[] = []) => ({
    bind: (...next: unknown[]) => statement(query, next),
    all: async <T>() => ({ results: sqlite.prepare(query).all(...(values as never[])) as T[] }),
    first: async <T>() => (sqlite.prepare(query).get(...(values as never[])) ?? null) as T | null,
    run: async () => sqlite.prepare(query).run(...(values as never[])),
  });
  return {
    sqlite,
    prepare: (query) => statement(query),
    batch: async (statements) => Promise.all(statements.map((s) => s.run())),
  };
}

/** A model that translates each string to `译：<string>`. */
function model(answer?: (texts: string[]) => unknown): Ai & { run: ReturnType<typeof vi.fn> } {
  return {
    run: vi.fn(async (_: string, input: { messages: { content: string }[] }) => {
      const texts = JSON.parse(input.messages[1].content.replace(" /no_think", "")) as string[];
      if (answer) return answer(texts);
      return {
        choices: [{ message: { content: `<think>\n</think>\n${JSON.stringify(texts.map((t) => `译：${t}`))}` } }],
        usage: { prompt_tokens: 100, completion_tokens: 100 },
      };
    }),
  };
}

const modrinth = (projects: Record<string, string>) =>
  vi.fn<typeof fetch>(async () =>
    Response.json(Object.entries(projects).map(([id, description]) => ({ id, description }))),
  );

const post = (platform: string, body: unknown) =>
  new Request(`${SITE}/translate/${platform}`, { method: "POST", body: JSON.stringify(body) });

beforeEach(() => resetSchema());

describe("readIds", () => {
  it("accepts MCIM's two request shapes only", () => {
    expect(readIds("modrinth", { project_ids: ["AANobbMI", "AANobbMI"] })).toEqual(["AANobbMI"]);
    expect(readIds("curseforge", { modids: [238222] })).toEqual(["238222"]);
    for (const body of [
      { project_ids: [] },
      { project_ids: ["../x"] },
      { project_ids: [1] },
      { modids: ["238222"] },
      { project_ids: Array.from({ length: MAX_IDS + 1 }, (_, i) => `id${i}`) },
      { text: "translate me" },
      null,
    ])
      expect(readIds("modrinth", body) ?? readIds("curseforge", body), JSON.stringify(body)).toBeNull();
  });
});

describe("translate", () => {
  it("translates the platform's own summaries once and serves them from D1 after", async () => {
    const env = { DB: d1(), AI: model() };
    const fetcher = modrinth({ AANobbMI: "Fast rendering", P7dR8mSH: "Fabric API" });
    const ask = () => translate(post("modrinth", { project_ids: ["AANobbMI", "P7dR8mSH"] }), env, fetcher);

    const first = await ask();
    expect(first.status).toBe(200);
    expect(await first.json()).toEqual([
      { project_id: "AANobbMI", original: "Fast rendering", translated: "译：Fast rendering" },
      { project_id: "P7dR8mSH", original: "Fabric API", translated: "译：Fabric API" },
    ]);
    expect(env.AI.run).toHaveBeenCalledTimes(1);
    expect(env.AI.run.mock.calls[0][0]).toBe(MODEL);
    expect(String(fetcher.mock.calls[0][0])).toBe(
      `https://api.modrinth.com/v2/projects?ids=${encodeURIComponent('["AANobbMI","P7dR8mSH"]')}`,
    );

    expect(await (await ask()).json()).toHaveLength(2);
    expect(env.AI.run).toHaveBeenCalledTimes(1);
  });

  it("translates again when the summary changes", async () => {
    const env = { DB: d1(), AI: model() };
    await translate(post("modrinth", { project_ids: ["AANobbMI"] }), env, modrinth({ AANobbMI: "Old" }));
    const response = await translate(post("modrinth", { project_ids: ["AANobbMI"] }), env, modrinth({ AANobbMI: "New" }));
    expect(await response.json()).toEqual([{ project_id: "AANobbMI", original: "New", translated: "译：New" }]);
    expect(env.AI.run).toHaveBeenCalledTimes(2);
  });

  it("answers CurseForge by numeric mod ID with the secret key", async () => {
    const env = { DB: d1(), AI: model(), CF_API_KEY: "key" };
    const fetcher = vi.fn<typeof fetch>(async (_, init) => {
      expect(new Headers(init?.headers).get("x-api-key")).toBe("key");
      expect(JSON.parse(String(init?.body))).toEqual({ modIds: [238222] });
      return Response.json({ data: [{ id: 238222, summary: "View items" }] });
    });
    const response = await translate(post("curseforge", { modids: [238222] }), env, fetcher);
    expect(await response.json()).toEqual([{ modid: 238222, original: "View items", translated: "译：View items" }]);

    const keyless = await translate(post("curseforge", { modids: [238222] }), { DB: d1(), AI: model() }, fetcher);
    expect(await keyless.json()).toEqual([]);
  });

  it("stops calling the model once the day's budget is spent, but still serves saved rows", async () => {
    const env = { DB: d1(), AI: model() };
    const fetcher = modrinth({ AANobbMI: "Saved", P7dR8mSH: "New" });
    await translate(post("modrinth", { project_ids: ["AANobbMI"] }), env, fetcher);
    const day = new Date().toISOString().slice(0, 10);
    env.DB.sqlite.prepare("UPDATE budget SET neurons = ? WHERE day = ?").run(DAILY_NEURONS, day);

    const response = await translate(post("modrinth", { project_ids: ["AANobbMI", "P7dR8mSH"] }), env, fetcher);
    expect(await response.json()).toEqual([{ project_id: "AANobbMI", original: "Saved", translated: "译：Saved" }]);
    expect(env.AI.run).toHaveBeenCalledTimes(1);
  });

  it("counts what each call cost", async () => {
    const env = { DB: d1(), AI: model() };
    await translate(post("modrinth", { project_ids: ["AANobbMI"] }), env, modrinth({ AANobbMI: "Text" }));
    const { neurons } = env.DB.sqlite.prepare("SELECT neurons FROM budget").get() as { neurons: number };
    expect(neurons).toBeCloseTo(100 * (4625 / 1e6) + 100 * (30475 / 1e6));
  });

  it("keeps nothing when the model's answer does not line up", async () => {
    for (const answer of [
      { response: '["only one"]', usage: {} },
      { response: '["a", ""]', usage: {} },
      { response: "Sure! Here you go.", usage: {} },
      { choices: [{ message: { content: '["a", 2]' } }] },
    ]) {
      resetSchema();
      const env = { DB: d1(), AI: model(() => answer) };
      const response = await translate(
        post("modrinth", { project_ids: ["AANobbMI", "P7dR8mSH"] }),
        env,
        modrinth({ AANobbMI: "One", P7dR8mSH: "Two" }),
      );
      expect(await response.json(), JSON.stringify(answer)).toEqual([]);
      expect(env.DB.sqlite.prepare("SELECT count(*) AS n FROM summaries").get()).toEqual({ n: 0 });
    }
  });

  it("returns no rows when the platform cannot be reached, and never translates unknown IDs", async () => {
    const env = { DB: d1(), AI: model() };
    const down = vi.fn<typeof fetch>(async () => new Response("", { status: 503 }));
    expect(await (await translate(post("modrinth", { project_ids: ["AANobbMI"] }), env, down)).json()).toEqual([]);
    // Modrinth also answers to slugs; only the IDs that were asked for count.
    const bySlug = modrinth({ OtherId1: "Someone else" });
    expect(await (await translate(post("modrinth", { project_ids: ["sodium"] }), env, bySlug)).json()).toEqual([]);
    expect(env.AI.run).not.toHaveBeenCalled();
  });

  it("refuses other methods, platforms and shapes", async () => {
    const env = { DB: d1(), AI: model() };
    const fetcher = vi.fn<typeof fetch>();
    expect((await translate(new Request(`${SITE}/translate/modrinth`), env, fetcher)).status).toBe(405);
    expect((await translate(post("github", { project_ids: ["a"] }), env, fetcher)).status).toBe(404);
    expect((await translate(post("modrinth", { text: "hi" }), env, fetcher)).status).toBe(400);
    expect((await translate(post("modrinth", { project_ids: ["a"] }), {}, fetcher)).status).toBe(503);
    expect(fetcher).not.toHaveBeenCalled();
  });
});
