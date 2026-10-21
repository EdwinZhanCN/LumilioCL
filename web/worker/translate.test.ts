import { readdirSync, readFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { describe, expect, it, vi } from "vitest";
import {
  type Ai,
  CLAIM_SECONDS,
  chinese,
  consume,
  DAILY_NEURONS,
  type Database,
  type Job,
  MAX_IDS,
  MODEL,
  readIds,
  readTranslations,
  translate,
} from "./translate";

const SITE = "https://launcher.lumilio.org";

const MIGRATIONS = new URL("../migrations/", import.meta.url);

/** A D1 binding over an in-memory SQLite database, with the migrations applied. */
function d1(): Database & { sqlite: DatabaseSync } {
  const sqlite = new DatabaseSync(":memory:");
  for (const file of readdirSync(MIGRATIONS).filter((name) => name.endsWith(".sql")).sort())
    sqlite.exec(readFileSync(new URL(file, MIGRATIONS), "utf8"));
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
      const texts = JSON.parse(input.messages[1].content) as string[];
      if (answer) return answer(texts);
      return {
        choices: [{ message: { content: JSON.stringify({ translations: texts.map((t) => `译：${t}`) }) } }],
        usage: { prompt_tokens: 100, completion_tokens: 100 },
      };
    }),
  };
}

/** A queue that keeps what was sent. */
const queue = () => {
  const sent: Job[] = [];
  return { sent, send: vi.fn(async (job: Job) => sent.push(job)) };
};

type Env = { DB: ReturnType<typeof d1>; AI: ReturnType<typeof model>; QUEUE: ReturnType<typeof queue>; CF_API_KEY?: string };
const environment = (ai = model()): Env => ({ DB: d1(), AI: ai, QUEUE: queue() });

/** Runs the queue consumer over every job sent so far, as Workers would. */
async function drain(env: Env, fetcher: typeof fetch, now?: Date) {
  const jobs = env.QUEUE.sent.splice(0);
  const ack = vi.fn();
  await consume({ messages: jobs.map((body) => ({ body, ack })) }, env, fetcher, now);
  expect(ack).toHaveBeenCalledTimes(jobs.length);
}

const modrinth = (projects: Record<string, string>) =>
  vi.fn<typeof fetch>(async () =>
    Response.json(Object.entries(projects).map(([id, description]) => ({ id, description }))),
  );

const post = (platform: string, body: unknown) =>
  new Request(`${SITE}/translate/${platform}`, { method: "POST", body: JSON.stringify(body) });

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

describe("chinese", () => {
  it("tells summaries already in Chinese from English ones that mention Chinese", () => {
    expect(chinese("厌倦了每次开局都是一样的钻石剑和弓？BattleArmory 让你每场重生都像开盲盒")).toBe(true);
    expect(chinese("僵尸入侵 100 天 —— Same as Forge Labs 100 Days Zombie Apocalypse in new Minecraft")).toBe(false);
    expect(chinese("View Items and Recipes")).toBe(false);
  });
});

describe("readTranslations", () => {
  it("reads the schema's object, a bare array, or an array in prose", () => {
    for (const content of ['{"translations": ["甲", "乙"]}', '<think>\n</think>\n["甲", "乙"]', 'Here: ["甲", "乙"]'])
      expect(readTranslations({ choices: [{ message: { content } }] }, 2).texts, content).toEqual(["甲", "乙"]);
    expect(readTranslations({ response: { translations: ["甲", "乙"] } }, 2).texts).toEqual(["甲", "乙"]);
  });
});

describe("translate", () => {
  it("queues what it has no rows for, and serves the consumer's translations after", async () => {
    const env = environment();
    const fetcher = modrinth({ AANobbMI: "Fast rendering", P7dR8mSH: "Fabric API" });
    const ask = () => translate(post("modrinth", { project_ids: ["AANobbMI", "P7dR8mSH"] }), env, fetcher);

    const first = await ask();
    expect(first.status).toBe(200);
    expect(await first.json()).toEqual([]);
    expect(env.AI.run).not.toHaveBeenCalled();
    expect(env.QUEUE.sent).toEqual([{ platform: "modrinth", ids: ["AANobbMI", "P7dR8mSH"] }]);

    await drain(env, fetcher);
    expect(env.AI.run).toHaveBeenCalledTimes(1);
    expect(env.AI.run.mock.calls[0][0]).toBe(MODEL);
    expect(String(fetcher.mock.calls[0][0])).toBe(
      `https://api.modrinth.com/v2/projects?ids=${encodeURIComponent('["AANobbMI","P7dR8mSH"]')}`,
    );

    expect(await (await ask()).json()).toEqual([
      { project_id: "AANobbMI", original: "Fast rendering", translated: "译：Fast rendering" },
      { project_id: "P7dR8mSH", original: "Fabric API", translated: "译：Fabric API" },
    ]);
    expect(env.QUEUE.sent).toEqual([]);
    expect(env.DB.sqlite.prepare("SELECT count(*) AS n FROM queued").get()).toEqual({ n: 0 });
  });

  it("asks the model to reason and to answer in the schema's shape", async () => {
    const env = environment();
    const fetcher = modrinth({ AANobbMI: "Fast rendering" });
    await translate(post("modrinth", { project_ids: ["AANobbMI"] }), env, fetcher);
    await drain(env, fetcher);
    const input = env.AI.run.mock.calls[0][1];
    expect(input.response_format.json_schema.schema.required).toEqual(["translations"]);
    expect(input.chat_template_kwargs).toBeUndefined();
    expect(input.messages[1].content).toBe('["Fast rendering"]');
  });

  it("queues a summary once while a job holds it, and again once the claim lapses", async () => {
    const env = environment();
    const fetcher = modrinth({ AANobbMI: "Fast rendering" });
    const at = (seconds: number) => new Date(Date.UTC(2026, 9, 21) + seconds * 1000);
    const ask = (seconds: number) => translate(post("modrinth", { project_ids: ["AANobbMI"] }), env, fetcher, at(seconds));

    await ask(0);
    await ask(60);
    expect(env.QUEUE.send).toHaveBeenCalledTimes(1);
    // The job failed: nothing saved, and the claim stands until it lapses.
    env.QUEUE.sent.splice(0);
    await ask(CLAIM_SECONDS - 1);
    expect(env.QUEUE.send).toHaveBeenCalledTimes(1);
    await ask(CLAIM_SECONDS);
    expect(env.QUEUE.send).toHaveBeenCalledTimes(2);
  });

  it("translates again when the summary changes, and forgets the old row", async () => {
    const env = environment();
    for (const text of ["Old", "New"]) {
      const fetcher = modrinth({ AANobbMI: text });
      await translate(post("modrinth", { project_ids: ["AANobbMI"] }), env, fetcher);
      await drain(env, fetcher);
    }
    const response = await translate(post("modrinth", { project_ids: ["AANobbMI"] }), env, modrinth({ AANobbMI: "New" }));
    expect(await response.json()).toEqual([{ project_id: "AANobbMI", original: "New", translated: "译：New" }]);
    expect(env.AI.run).toHaveBeenCalledTimes(2);
    expect(env.DB.sqlite.prepare("SELECT count(*) AS n FROM summaries").get()).toEqual({ n: 1 });
  });

  it("does not serve rows saved under another model or prompt", async () => {
    const env = environment();
    const fetcher = modrinth({ AANobbMI: "Fast rendering" });
    const plain = [...new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode("Fast rendering")))]
      .map((byte) => byte.toString(16).padStart(2, "0"))
      .join("");
    env.DB.sqlite.prepare("INSERT INTO summaries VALUES ('modrinth', 'AANobbMI', ?, '旧译文', 0)").run(plain);
    expect(await (await translate(post("modrinth", { project_ids: ["AANobbMI"] }), env, fetcher)).json()).toEqual([]);
    expect(env.QUEUE.sent).toHaveLength(1);
  });

  it("leaves summaries already in Chinese alone", async () => {
    const env = environment();
    const fetcher = modrinth({ rQiXwLhB: "厌倦了每次开局都是一样的钻石剑和弓？" });
    expect(await (await translate(post("modrinth", { project_ids: ["rQiXwLhB"] }), env, fetcher)).json()).toEqual([]);
    expect(env.QUEUE.send).not.toHaveBeenCalled();
  });

  it("answers CurseForge by numeric mod ID with the secret key", async () => {
    const env = { ...environment(), CF_API_KEY: "key" };
    const fetcher = vi.fn<typeof fetch>(async (_, init) => {
      expect(new Headers(init?.headers).get("x-api-key")).toBe("key");
      expect(JSON.parse(String(init?.body))).toEqual({ modIds: [238222] });
      return Response.json({ data: [{ id: 238222, summary: "View items" }] });
    });
    await translate(post("curseforge", { modids: [238222] }), env, fetcher);
    expect(env.QUEUE.sent).toEqual([{ platform: "curseforge", ids: ["238222"] }]);
    await drain(env, fetcher);
    const response = await translate(post("curseforge", { modids: [238222] }), env, fetcher);
    expect(await response.json()).toEqual([{ modid: 238222, original: "View items", translated: "译：View items" }]);

    const keyless = environment();
    expect(await (await translate(post("curseforge", { modids: [238222] }), keyless, fetcher)).json()).toEqual([]);
    expect(keyless.QUEUE.send).not.toHaveBeenCalled();
  });

  it("stops queueing and translating once the day's budget is spent, but still serves saved rows", async () => {
    const env = environment();
    const fetcher = modrinth({ AANobbMI: "Saved", P7dR8mSH: "New" });
    await translate(post("modrinth", { project_ids: ["AANobbMI"] }), env, fetcher);
    await drain(env, fetcher);
    // A job queued before the budget ran out.
    env.QUEUE.sent.push({ platform: "modrinth", ids: ["P7dR8mSH"] });
    const day = new Date().toISOString().slice(0, 10);
    env.DB.sqlite.prepare("UPDATE budget SET neurons = ? WHERE day = ?").run(DAILY_NEURONS, day);

    const response = await translate(post("modrinth", { project_ids: ["AANobbMI", "P7dR8mSH"] }), env, fetcher);
    expect(await response.json()).toEqual([{ project_id: "AANobbMI", original: "Saved", translated: "译：Saved" }]);
    expect(env.QUEUE.send).toHaveBeenCalledTimes(1);
    await drain(env, fetcher);
    expect(env.AI.run).toHaveBeenCalledTimes(1);
  });

  it("counts what each call cost, reasoning included", async () => {
    const env = environment();
    const fetcher = modrinth({ AANobbMI: "Text" });
    await translate(post("modrinth", { project_ids: ["AANobbMI"] }), env, fetcher);
    await drain(env, fetcher);
    const { neurons } = env.DB.sqlite.prepare("SELECT neurons FROM budget").get() as { neurons: number };
    expect(neurons).toBeCloseTo(100 * (5500 / 1e6) + 100 * (36400 / 1e6));
  });

  it("keeps nothing when the model's answer does not line up", async () => {
    for (const answer of [
      { response: '["only one"]', usage: {} },
      { response: '["a", ""]', usage: {} },
      { response: "Sure! Here you go.", usage: {} },
      { choices: [{ message: { content: '{"translations": ["a", 2]}' } }] },
      { choices: [{ message: { content: '{"translations": ["a", "b"' } }] },
    ]) {
      const env = environment(model(() => answer));
      const fetcher = modrinth({ AANobbMI: "One", P7dR8mSH: "Two" });
      await translate(post("modrinth", { project_ids: ["AANobbMI", "P7dR8mSH"] }), env, fetcher);
      await drain(env, fetcher);
      const response = await translate(post("modrinth", { project_ids: ["AANobbMI", "P7dR8mSH"] }), env, fetcher);
      expect(await response.json(), JSON.stringify(answer)).toEqual([]);
      expect(env.DB.sqlite.prepare("SELECT count(*) AS n FROM summaries").get()).toEqual({ n: 0 });
    }
  });

  it("returns no rows when the platform cannot be reached, and never translates unknown IDs", async () => {
    const env = environment();
    const down = vi.fn<typeof fetch>(async () => new Response("", { status: 503 }));
    expect(await (await translate(post("modrinth", { project_ids: ["AANobbMI"] }), env, down)).json()).toEqual([]);
    // Modrinth also answers to slugs; only the IDs that were asked for count.
    const bySlug = modrinth({ OtherId1: "Someone else" });
    expect(await (await translate(post("modrinth", { project_ids: ["sodium"] }), env, bySlug)).json()).toEqual([]);
    expect(env.QUEUE.send).not.toHaveBeenCalled();
    // A job naming IDs the platform no longer knows translates nothing.
    env.QUEUE.sent.push({ platform: "modrinth", ids: ["sodium"] }, { platform: "github", ids: ["a"] } as never);
    await drain(env, bySlug);
    expect(env.AI.run).not.toHaveBeenCalled();
  });

  it("refuses other methods, platforms and shapes", async () => {
    const env = environment();
    const fetcher = vi.fn<typeof fetch>();
    expect((await translate(new Request(`${SITE}/translate/modrinth`), env, fetcher)).status).toBe(405);
    expect((await translate(post("github", { project_ids: ["a"] }), env, fetcher)).status).toBe(404);
    expect((await translate(post("modrinth", { text: "hi" }), env, fetcher)).status).toBe(400);
    expect((await translate(post("modrinth", { project_ids: ["a"] }), {}, fetcher)).status).toBe(503);
    expect(fetcher).not.toHaveBeenCalled();
  });
});
