// Chinese project summaries for Discover, when MCIM has none (plan
// translation-fallback). The request and the rows match MCIM's
// /translate/{modrinth,curseforge} (https://github.com/mcmod-info-mirror/mcim-rust-api,
// src/routes/translate.rs), so the launcher reads both the same way.
//
// Only project IDs come in (D1): the summaries are fetched from Modrinth or
// CurseForge here, so this never translates text a caller chose. Each summary
// is translated once with Workers AI and kept in D1 until it changes; a daily
// neuron budget keeps the model inside the free allocation, after which only
// saved rows are returned.

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

export type TranslateEnv = { AI?: Ai; DB?: Database; CF_API_KEY?: string };

export const MODEL = "@cf/qwen/qwen3-30b-a3b-fp8";
// Neurons per token for MODEL (Workers AI pricing page, 2026-10-19).
const INPUT_NEURONS = 4625 / 1e6;
const OUTPUT_NEURONS = 30475 / 1e6;
/** Below the free 10,000 a day, so one late batch cannot run over. */
export const DAILY_NEURONS = 9000;
export const MAX_IDS = 50;
// Summaries per model call, and the longest summary worth sending.
const BATCH = 25;
const MAX_SUMMARY = 1000;

const USER_AGENT = "EdwinZhanCN/LumilioCL (launcher.lumilio.org)";

const PROMPT = `You translate short summaries of Minecraft projects (mods, modpacks, resource packs, shaders, data packs, plugins) from English into Simplified Chinese.
Keep names of projects, mods, games and companies, file names, version numbers, URLs, commands and code as written.
Use the terms the Chinese Minecraft community uses, for example: mod 模组, modpack 整合包, resource pack 资源包, shader 光影, data pack 数据包, the Overworld 主世界, the Nether 下界, the End 末地, mob 生物, block 方块, item 物品, enchantment 附魔, biome 生物群系, villager 村民.
The input is a JSON array of strings. Answer with only a JSON array of the same length, each element the translation of the string at the same position. No notes, no explanations.`;

type Row = { id: string; original: string; translated: string };

// The tables (summaries, budget) are in migrations/0001_translations.sql.

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

async function sha256(text: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
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
  if (Array.isArray(raw)) return check(raw, count, neurons);
  if (typeof raw !== "string") return { texts: [], neurons };
  const content = raw.replace(/<think>[\s\S]*?<\/think>/g, "");
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
  if (!env.DB) return json({ error: "unavailable" }, 503);
  let body: unknown;
  try {
    body = await request.json();
  } catch {
    return json({ error: "bad request" }, 400);
  }
  const ids = readIds(platform, body);
  if (!ids) return json({ error: "bad request" }, 400);

  const db = env.DB;
  const originals = await summaries(platform, ids, env, fetcher);
  const wanted = [...originals].filter(([, text]) => text.trim() !== "" && text.length <= MAX_SUMMARY);
  const hashes = new Map(await Promise.all(wanted.map(async ([id, text]) => [id, await sha256(text)] as const)));

  const rows: Row[] = [];
  const missing: [string, string][] = [];
  if (wanted.length > 0) {
    const saved = await db
      .prepare(
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

  if (missing.length > 0 && env.AI) {
    const day = now.toISOString().slice(0, 10);
    for (let start = 0; start < missing.length; start += BATCH) {
      const spent = await db.prepare("SELECT neurons FROM budget WHERE day = ?").bind(day).first<{ neurons: number }>();
      if ((spent?.neurons ?? 0) >= DAILY_NEURONS) break;
      const batch = missing.slice(start, start + BATCH);
      let output: unknown;
      try {
        output = await env.AI.run(MODEL, {
          messages: [
            { role: "system", content: PROMPT },
            { role: "user", content: `${JSON.stringify(batch.map(([, text]) => text))} /no_think` },
          ],
          max_tokens: 4000,
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
          .bind(day, result.neurons),
      ];
      result.texts.forEach((translated, index) => {
        const [id, original] = batch[index];
        rows.push({ id, original, translated });
        statements.push(
          db
            .prepare(
              "INSERT OR REPLACE INTO summaries (platform, id, hash, translated, created) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(platform, id, hashes.get(id), translated, Math.floor(now.getTime() / 1000)),
        );
      });
      await db.batch(statements);
    }
  }

  return json(
    rows.map(({ id, original, translated }) =>
      platform === "modrinth" ? { project_id: id, original, translated } : { modid: Number(id), original, translated },
    ),
  );
}
