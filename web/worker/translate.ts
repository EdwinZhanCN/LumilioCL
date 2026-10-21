// Chinese project summaries for Discover, when MCIM has none (plan
// translation-fallback). The request and the rows match MCIM's
// /translate/{modrinth,curseforge} (https://github.com/mcmod-info-mirror/mcim-rust-api,
// src/routes/translate.rs), so the launcher reads both the same way.
//
// Only project IDs come in (D1): the summaries are fetched from Modrinth or
// CurseForge here, so this never translates text a caller chose. A request
// answers with the rows already in D1 and queues the rest (D6): the model
// reasons before it answers, which takes longer than the launcher waits, so
// the queue consumer translates and saves them, and the launcher's next ask
// finds them. Each summary is translated once and kept until it changes; a
// daily neuron budget keeps the model inside the free allocation.

export type Platform = "modrinth" | "curseforge";

/** The parts of the Workers AI binding used here. */
export type Ai = { run(model: string, input: unknown): Promise<unknown> };

/** The parts of a D1 binding used here. */
export type Database = {
  prepare(query: string): Statement;
  batch(statements: Statement[]): Promise<unknown>;
};
export type Statement = {
  bind(...values: unknown[]): Statement;
  all<T>(): Promise<{ results: T[] }>;
  first<T>(): Promise<T | null>;
  run(): Promise<unknown>;
};

/** One queued job. IDs only: summaries are fetched again when it runs (D5). */
export type Job = { platform: Platform; ids: string[] };
/** The parts of a Queues producer binding used here. */
export type Queue = { send(body: Job): Promise<unknown> };
/** The parts of a delivered Queues batch used here. */
export type Batch = { messages: readonly { body: unknown; ack(): void }[] };

export type TranslateEnv = { AI?: Ai; DB?: Database; QUEUE?: Queue; CF_API_KEY?: string };

export const MODEL = "@cf/zai-org/glm-4.7-flash";
// Neurons per token for MODEL, reasoning tokens included (Workers AI pricing
// page, 2026-10-21).
const INPUT_NEURONS = 5500 / 1e6;
const OUTPUT_NEURONS = 36400 / 1e6;
/** Hashed with each original, so a new model or prompt translates every summary again. */
export const VERSION = `${MODEL} prompt-2`;
/** Below the free 10,000 a day, so one late batch cannot run over. */
export const DAILY_NEURONS = 9000;
export const MAX_IDS = 50;
// Summaries per model call, and the longest summary worth sending.
const BATCH = 25;
const MAX_SUMMARY = 1000;
// Reasoning took about 4,000 tokens for 25 summaries.
const MAX_TOKENS = 16000;
/** How long a queued summary keeps other requests from queueing it again. */
export const CLAIM_SECONDS = 15 * 60;

const USER_AGENT = "EdwinZhanCN/LumilioCL (launcher.lumilio.org)";

const PROMPT = `You translate short summaries of Minecraft projects (mods, modpacks, resource packs, shaders, data packs, plugins) from English into Simplified Chinese.
Keep names of projects, mods, modpacks, games and companies as written, without quotation or title marks: Minecraft stays Minecraft, Farmer's Delight stays Farmer's Delight. Also keep file names, version numbers, URLs, commands and code as written.
Use the terms the Chinese Minecraft community uses, for example: mod 模组, modpack 整合包, resource pack 资源包, shader 光影, data pack 数据包, addon 附属模组, the Overworld 主世界, the Nether 下界, the End 末地, Nether portal 下界传送门, Netherite 下界合金, mob 生物, block 方块, item 物品, inventory 物品栏, enchantment 附魔, biome 生物群系, villager 村民, Vanilla 原版, Vanilla+ 原版+, overhaul 重制, lag 卡顿, endgame 终局内容, path tracing 路径追踪.
Address the reader as 你. Put a space between Chinese and Latin letters or digits.
The input is a JSON array of strings. Answer with a JSON object {"translations": [...]} whose array has the same length, each element the translation of the string at the same position.`;

// Workers AI constrains the answer to this shape.
const RESPONSE_FORMAT = {
  type: "json_schema",
  json_schema: {
    name: "translations",
    strict: true,
    schema: {
      type: "object",
      properties: { translations: { type: "array", items: { type: "string" } } },
      required: ["translations"],
      additionalProperties: false,
    },
  },
};

type Row = { id: string; original: string; translated: string };

// The tables (summaries, budget, queued) are in migrations/.

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json", "cache-control": "no-store" },
  });

/** The requested IDs, or null when the body is not MCIM's shape. */
export function readIds(platform: Platform, body: unknown): string[] | null {
  if (typeof body !== "object" || body === null) return null;
  const list = (body as Record<string, unknown>)[platform === "modrinth" ? "project_ids" : "modids"];
  if (!Array.isArray(list) || list.length === 0 || list.length > MAX_IDS) return null;
  const valid =
    platform === "modrinth"
      ? list.every((id) => typeof id === "string" && /^[0-9A-Za-z]{1,32}$/.test(id))
      : list.every((id) => Number.isSafeInteger(id) && id > 0 && id <= 0x7fffffff);
  if (!valid) return null;
  return [...new Set(list.map(String))];
}

/** Current summaries by ID from the platform; empty when it cannot be asked. */
export async function summaries(
  platform: Platform,
  ids: string[],
  env: TranslateEnv,
  fetcher: typeof fetch,
): Promise<Map<string, string>> {
  const found = new Map<string, string>();
  try {
    if (platform === "modrinth") {
      const url = `https://api.modrinth.com/v2/projects?ids=${encodeURIComponent(JSON.stringify(ids))}`;
      const response = await fetcher(url, { headers: { "user-agent": USER_AGENT } });
      if (!response.ok) return found;
      for (const project of (await response.json()) as { id?: unknown; description?: unknown }[])
        if (typeof project.id === "string" && typeof project.description === "string")
          found.set(project.id, project.description);
    } else {
      if (!env.CF_API_KEY) return found;
      const response = await fetcher("https://api.curseforge.com/v1/mods", {
        method: "POST",
        headers: { "user-agent": USER_AGENT, "content-type": "application/json", "x-api-key": env.CF_API_KEY },
        body: JSON.stringify({ modIds: ids.map(Number) }),
      });
      if (!response.ok) return found;
      const { data } = (await response.json()) as { data?: { id?: unknown; summary?: unknown }[] };
      for (const mod of data ?? [])
        if (Number.isSafeInteger(mod.id) && typeof mod.summary === "string") found.set(String(mod.id), mod.summary);
    }
  } catch {
    return new Map();
  }
  // Only what was asked for: Modrinth also answers to slugs.
  for (const id of found.keys()) if (!ids.includes(id)) found.delete(id);
  return found;
}

/** Mostly Han already: shown as written, never sent to the model. */
export function chinese(text: string): boolean {
  const han = text.match(/\p{Script=Han}/gu)?.length ?? 0;
  return han > 0 && han * 2 >= (text.match(/\p{L}/gu)?.length ?? 0);
}

async function sha256(text: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

/** The summaries worth translating, the hash each is saved under, and which are saved already. */
async function lookup(platform: Platform, ids: string[], env: TranslateEnv & { DB: Database }, fetcher: typeof fetch) {
  const originals = await summaries(platform, ids, env, fetcher);
  const wanted = [...originals].filter(
    ([, text]) => text.trim() !== "" && text.length <= MAX_SUMMARY && !chinese(text),
  );
  const hashes = new Map(
    await Promise.all(wanted.map(async ([id, text]) => [id, await sha256(`${VERSION}\n${text}`)] as const)),
  );
  const rows: Row[] = [];
  const missing: [string, string][] = [];
  if (wanted.length > 0) {
    const saved = await env.DB.prepare(
      `SELECT id, hash, translated FROM summaries WHERE platform = ? AND id IN (${wanted.map(() => "?").join(", ")})`,
    )
      .bind(platform, ...wanted.map(([id]) => id))
      .all<{ id: string; hash: string; translated: string }>();
    const byId = new Map(saved.results.filter((row) => row.hash === hashes.get(row.id)).map((row) => [row.id, row]));
    for (const [id, original] of wanted) {
      const row = byId.get(id);
      if (row) rows.push({ id, original, translated: row.translated });
      else missing.push([id, original]);
    }
  }
  return { hashes, rows, missing };
}

const today = (now: Date) => now.toISOString().slice(0, 10);

async function spent(db: Database, now: Date): Promise<number> {
  const row = await db.prepare("SELECT neurons FROM budget WHERE day = ?").bind(today(now)).first<{ neurons: number }>();
  return row?.neurons ?? 0;
}

/** Marks summaries as queued and returns the IDs no recent job holds. */
async function claim(db: Database, platform: Platform, entries: [string, string][], now: Date): Promise<string[]> {
  const at = Math.floor(now.getTime() / 1000);
  await db.prepare("DELETE FROM queued WHERE claimed <= ?").bind(at - CLAIM_SECONDS).run();
  const { results } = await db
    .prepare(
      `INSERT INTO queued (platform, id, hash, claimed)
       SELECT ?, json_extract(value, '$[0]'), json_extract(value, '$[1]'), ? FROM json_each(?) WHERE true
       ON CONFLICT DO NOTHING RETURNING id`,
    )
    .bind(platform, at, JSON.stringify(entries))
    .all<{ id: string }>();
  return results.map((row) => row.id);
}

/** The model's translations in order (none when they do not line up), and what the call cost. */
export function readTranslations(output: unknown, count: number): { texts: string[]; neurons: number } {
  const answer = output as {
    response?: unknown;
    choices?: { message?: { content?: unknown } }[];
    usage?: { prompt_tokens?: unknown; completion_tokens?: unknown };
  };
  const usage = answer?.usage;
  const tokens = (value: unknown) => (typeof value === "number" && value > 0 ? value : 0);
  const neurons = tokens(usage?.prompt_tokens) * INPUT_NEURONS + tokens(usage?.completion_tokens) * OUTPUT_NEURONS;
  const raw = answer?.choices?.[0]?.message?.content ?? answer?.response;
  const listed = (value: unknown) => (Array.isArray(value) ? value : (value as { translations?: unknown })?.translations);
  if (typeof raw === "object" && raw !== null) return check(listed(raw), count, neurons);
  if (typeof raw !== "string") return { texts: [], neurons };
  const content = raw.replace(/<think>[\s\S]*?<\/think>/g, "").trim();
  try {
    return check(listed(JSON.parse(content)), count, neurons);
  } catch {
    // An array somewhere in prose.
  }
  const start = content.indexOf("[");
  const end = content.lastIndexOf("]");
  if (start < 0 || end < start) return { texts: [], neurons };
  try {
    return check(JSON.parse(content.slice(start, end + 1)), count, neurons);
  } catch {
    return { texts: [], neurons };
  }
}

function check(texts: unknown, count: number, neurons: number) {
  if (!Array.isArray(texts) || texts.length !== count) return { texts: [], neurons };
  if (!texts.every((text) => typeof text === "string" && text.trim() !== "")) return { texts: [], neurons };
  return { texts: (texts as string[]).map((text) => text.trim()), neurons };
}

/** Handles POST /translate/<platform>. `fetcher` is injectable for tests. */
export async function translate(
  request: Request,
  env: TranslateEnv,
  fetcher: typeof fetch = fetch,
  now: Date = new Date(),
): Promise<Response> {
  const platform = new URL(request.url).pathname.slice("/translate/".length);
  if (platform !== "modrinth" && platform !== "curseforge") return json({ error: "not found" }, 404);
  if (request.method !== "POST") return new Response("Method not allowed", { status: 405, headers: { allow: "POST" } });
  const db = env.DB;
  if (!db) return json({ error: "unavailable" }, 503);
  let body: unknown;
  try {
    body = await request.json();
  } catch {
    return json({ error: "bad request" }, 400);
  }
  const ids = readIds(platform, body);
  if (!ids) return json({ error: "bad request" }, 400);

  const { hashes, rows, missing } = await lookup(platform, ids, { ...env, DB: db }, fetcher);
  if (missing.length > 0 && env.QUEUE && (await spent(db, now)) < DAILY_NEURONS) {
    const queued = await claim(db, platform, missing.map(([id]) => [id, hashes.get(id) ?? ""]), now);
    if (queued.length > 0) await env.QUEUE.send({ platform, ids: queued });
  }

  return json(
    rows.map(({ id, original, translated }) =>
      platform === "modrinth" ? { project_id: id, original, translated } : { modid: Number(id), original, translated },
    ),
  );
}

/** Handles the queue: translates and saves each job's summaries that are still missing. */
export async function consume(batch: Batch, env: TranslateEnv, fetcher: typeof fetch = fetch, now: Date = new Date()) {
  for (const message of batch.messages) {
    const { platform, ids } = (message.body ?? {}) as Partial<Job>;
    const valid =
      (platform === "modrinth" || platform === "curseforge") &&
      readIds(platform, platform === "modrinth" ? { project_ids: ids } : { modids: ids?.map(Number) });
    if (valid && env.DB && env.AI) await work(platform, valid, env.DB, env.AI, env, fetcher, now);
    // A failed batch is not retried here: its claim lapses and a later request queues it again.
    message.ack();
  }
}

async function work(
  platform: Platform,
  ids: string[],
  db: Database,
  ai: Ai,
  env: TranslateEnv,
  fetcher: typeof fetch,
  now: Date,
) {
  const { hashes, missing } = await lookup(platform, ids, { ...env, DB: db }, fetcher);
  for (let start = 0; start < missing.length; start += BATCH) {
    if ((await spent(db, now)) >= DAILY_NEURONS) break;
    const batch = missing.slice(start, start + BATCH);
    let output: unknown;
    try {
      output = await ai.run(MODEL, {
        messages: [
          { role: "system", content: PROMPT },
          { role: "user", content: JSON.stringify(batch.map(([, text]) => text)) },
        ],
        response_format: RESPONSE_FORMAT,
        max_tokens: MAX_TOKENS,
        temperature: 0.2,
      });
    } catch {
      break;
    }
    const result = readTranslations(output, batch.length);
    const statements = [
      db
        .prepare(
          "INSERT INTO budget (day, neurons) VALUES (?, ?) ON CONFLICT (day) DO UPDATE SET neurons = neurons + excluded.neurons",
        )
        .bind(today(now), result.neurons),
    ];
    result.texts.forEach((translated, index) => {
      const [id] = batch[index];
      const hash = hashes.get(id);
      // Rows of an older summary, model or prompt are never served again.
      statements.push(
        db.prepare("DELETE FROM summaries WHERE platform = ? AND id = ? AND hash <> ?").bind(platform, id, hash),
        db
          .prepare("INSERT OR REPLACE INTO summaries (platform, id, hash, translated, created) VALUES (?, ?, ?, ?, ?)")
          .bind(platform, id, hash, translated, Math.floor(now.getTime() / 1000)),
        db.prepare("DELETE FROM queued WHERE platform = ? AND id = ?").bind(platform, id),
      );
    });
    await db.batch(statements);
  }
}
