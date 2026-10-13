// The launcher's theme file in TypeScript, for the theme studio. The rules
// follow crates/lumilio-ui/src/theme/spec.rs; the built-in themes, colour
// names, radii and contrast rules come from Rust through
// themes.generated.json (`just website-themes`), never from copies here.
import generated from "../../data/themes.generated.json";

export type Tone = "light" | "dark";
export type WindowBackground = "opaque" | "transparent" | "blurred";

export interface Fonts {
  sans?: string;
  mono?: string;
  cjk?: string;
}

/** One theme as written in a file. Colours are normalised hex strings. */
export interface ThemeSpec {
  name: string;
  appearance: Tone;
  source?: string;
  colors: Record<string, string>;
  radius?: number;
  radius_lg?: number;
  fonts: Fonts;
  window_background?: WindowBackground;
}

export interface ContrastRule {
  id: string;
  fg: string;
  bg: string;
  light: number;
  dark: number;
}

export const COLOR_KEYS: readonly string[] = generated.colors;
export const DEFAULT_RADIUS: number = generated.radius;
export const DEFAULT_RADIUS_LG: number = generated.radius_lg;
export const MAX_RADIUS: number = generated.max_radius;
export const CONTRAST_RULES: readonly ContrastRule[] = generated.contrast;
export const BUILTIN_THEMES: readonly ThemeSpec[] = generated.themes.map((theme) => ({
  ...theme,
  appearance: theme.appearance as Tone,
  colors: theme.colors as Record<string, string>,
}));

/** The launcher's own themes, which fill what another theme leaves out. */
const BASE: Record<Tone, ThemeSpec> = {
  light: BUILTIN_THEMES.find((theme) => theme.name === "Aluminium")!,
  dark: BUILTIN_THEMES.find((theme) => theme.name === "Night")!,
};

// ---- Colours ----

export interface Rgba {
  r: number;
  g: number;
  b: number;
  /** 0–255. */
  a: number;
}

/** `#rrggbb` or `#rrggbbaa`, the `#` optional, as the launcher reads them. */
export function parseColor(text: string): Rgba | null {
  const digits = text.trim().replace(/^#+/, "");
  if (!/^[0-9a-fA-F]+$/.test(digits) || (digits.length !== 6 && digits.length !== 8)) return null;
  const byte = (at: number) => parseInt(digits.slice(at, at + 2), 16);
  return { r: byte(0), g: byte(2), b: byte(4), a: digits.length === 8 ? byte(6) : 255 };
}

/** How the launcher writes a colour: lower case, alpha only when not opaque. */
export function formatColor({ r, g, b, a }: Rgba): string {
  const hex = (value: number) => Math.round(value).toString(16).padStart(2, "0");
  return `#${hex(r)}${hex(g)}${hex(b)}${a === 255 ? "" : hex(a)}`;
}

export function normalizeColor(text: string): string | null {
  const color = parseColor(text);
  return color && formatColor(color);
}

function over(top: Rgba, bottom: Rgba): Rgba {
  const alpha = top.a / 255;
  const mix = (a: number, b: number) => a * alpha + b * (1 - alpha);
  return { r: mix(top.r, bottom.r), g: mix(top.g, bottom.g), b: mix(top.b, bottom.b), a: 255 };
}

function luminance({ r, g, b }: Rgba): number {
  const channel = (value: number) => {
    const c = value / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

/** WCAG 2 contrast ratio of two opaque colours. */
export function contrast(a: Rgba, b: Rgba): number {
  const [x, y] = [luminance(a), luminance(b)];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}

// ---- Themes ----

/** A theme with every gap filled: what the launcher draws with. */
export interface Resolved {
  tone: Tone;
  colors: Record<string, string>;
  /** Colours the theme does not give, filled from the built-in theme. */
  missing: string[];
  radius: number;
  radiusLg: number;
  fonts: Fonts;
  windowBackground: WindowBackground;
}

/** A radius the launcher accepts: finite, between 0 and the maximum. */
export function clampRadius(value: number | undefined, fallback: number): number {
  if (value === undefined || !Number.isFinite(value)) return fallback;
  return Math.min(Math.max(value, 0), MAX_RADIUS);
}

export function resolve(theme: ThemeSpec): Resolved {
  const base = BASE[theme.appearance];
  const colors: Record<string, string> = {};
  const missing: string[] = [];
  for (const key of COLOR_KEYS) {
    const own = theme.colors[key];
    if (own) colors[key] = own;
    else {
      colors[key] = base.colors[key];
      missing.push(key);
    }
  }
  return {
    tone: theme.appearance,
    colors,
    missing,
    radius: clampRadius(theme.radius, DEFAULT_RADIUS),
    radiusLg: clampRadius(theme.radius_lg, DEFAULT_RADIUS_LG),
    fonts: theme.fonts,
    windowBackground: theme.window_background ?? "opaque",
  };
}

export interface ContrastResult {
  rule: ContrastRule;
  ratio: number;
  floor: number;
  ok: boolean;
}

/** The launcher's contrast rules against a resolved theme. A translucent
 * colour is judged over what it sits on, and a translucent background over
 * the built-in page of its tone. */
export function checkContrast(theme: Resolved): ContrastResult[] {
  const page = parseColor(BASE[theme.tone].colors.page)!;
  const colour = (key: string) => parseColor(theme.colors[key])!;
  return CONTRAST_RULES.map((rule) => {
    const bg = over(colour(rule.bg), page);
    const ratio = contrast(over(colour(rule.fg), bg), bg);
    const floor = theme.tone === "dark" ? rule.dark : rule.light;
    return { rule, ratio, floor, ok: ratio >= floor };
  });
}

// ---- Files ----

export type Parsed = { ok: true; theme: ThemeSpec } | { ok: false; error: string };

const TONES: readonly string[] = ["light", "dark"];
const BACKGROUNDS: readonly string[] = ["opaque", "transparent", "blurred"];

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** One theme of a file, as the launcher reads it: unknown fields are
 * ignored, a bad known field rejects the theme. */
function parseTheme(value: unknown): Parsed {
  const name = isObject(value) && typeof value.name === "string" ? value.name : "?";
  const fail = (why: string): Parsed => ({ ok: false, error: `${name}: ${why}` });
  if (!isObject(value)) return fail("a theme is an object");
  if (typeof value.name !== "string") return fail("`name` is missing");
  if (typeof value.appearance !== "string" || !TONES.includes(value.appearance))
    return fail("`appearance` is \"light\" or \"dark\"");
  const theme: ThemeSpec = { name, appearance: value.appearance as Tone, colors: {}, fonts: {} };
  if (value.source !== undefined && value.source !== null) {
    if (typeof value.source !== "string") return fail("`source` is text");
    theme.source = value.source;
  }
  if (value.colors !== undefined) {
    if (!isObject(value.colors)) return fail("`colors` is an object");
    for (const key of COLOR_KEYS) {
      const text = value.colors[key];
      if (text === undefined || text === null) continue;
      const color = typeof text === "string" ? normalizeColor(text) : null;
      if (!color) return fail(`\`${String(text)}\` is not a colour (#rrggbb or #rrggbbaa)`);
      theme.colors[key] = color;
    }
  }
  for (const field of ["radius", "radius_lg"] as const) {
    const radius = value[field];
    if (radius === undefined || radius === null) continue;
    if (typeof radius !== "number") return fail(`\`${field}\` is a number`);
    theme[field] = radius;
  }
  if (value.fonts !== undefined) {
    if (!isObject(value.fonts)) return fail("`fonts` is an object");
    for (const slot of ["sans", "mono", "cjk"] as const) {
      const family = value.fonts[slot];
      if (family === undefined || family === null) continue;
      if (typeof family !== "string") return fail(`\`fonts.${slot}\` is text`);
      theme.fonts[slot] = family;
    }
  }
  const background = value.window_background;
  if (background !== undefined && background !== null) {
    if (typeof background !== "string" || !BACKGROUNDS.includes(background))
      return fail("`window_background` is \"opaque\", \"transparent\" or \"blurred\"");
    theme.window_background = background as WindowBackground;
  }
  return { ok: true, theme };
}

/** A theme file: the outer error is a file that is not a theme file at
 * all; each theme inside answers for itself. */
export function parseFile(text: string): { ok: true; themes: Parsed[] } | { ok: false; error: string } {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch (error) {
    return { ok: false, error: error instanceof Error ? error.message : String(error) };
  }
  if (!isObject(value) || !Array.isArray(value.themes)) return { ok: false, error: "missing field `themes`" };
  return { ok: true, themes: value.themes.map(parseTheme) };
}

/** A theme as the file writes it: fields in the launcher's order, colours in
 * palette order, and nothing that only repeats a default. */
function writeTheme(theme: ThemeSpec): Record<string, unknown> {
  const colors: Record<string, string> = {};
  for (const key of COLOR_KEYS) if (theme.colors[key]) colors[key] = theme.colors[key];
  const fonts: Fonts = {};
  for (const slot of ["sans", "mono", "cjk"] as const) {
    const family = theme.fonts[slot]?.trim();
    if (family) fonts[slot] = family;
  }
  return {
    name: theme.name,
    appearance: theme.appearance,
    ...(theme.source?.trim() ? { source: theme.source.trim() } : {}),
    colors,
    ...(theme.radius !== undefined ? { radius: theme.radius } : {}),
    ...(theme.radius_lg !== undefined ? { radius_lg: theme.radius_lg } : {}),
    ...(Object.keys(fonts).length ? { fonts } : {}),
    ...(theme.window_background && theme.window_background !== "opaque"
      ? { window_background: theme.window_background }
      : {}),
  };
}

export function writeFile(themes: readonly ThemeSpec[]): string {
  return `${JSON.stringify({ themes: themes.map(writeTheme) }, null, 2)}\n`;
}

/** A file name from the first theme's name. */
export function fileName(themes: readonly ThemeSpec[]): string {
  const slug = (themes[0]?.name ?? "")
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
  return `${slug || "theme"}.json`;
}

export type Problem =
  | { kind: "no-name"; index: number }
  | { kind: "duplicate"; index: number; of: number }
  | { kind: "builtin"; index: number };

/** What would keep the launcher from listing a theme of this file. */
export function problems(themes: readonly ThemeSpec[]): Problem[] {
  const found: Problem[] = [];
  themes.forEach((theme, index) => {
    if (!theme.name.trim()) {
      found.push({ kind: "no-name", index });
      return;
    }
    const same = (other: ThemeSpec) => other.name === theme.name && other.appearance === theme.appearance;
    if (BUILTIN_THEMES.some(same)) found.push({ kind: "builtin", index });
    const earlier = themes.findIndex(same);
    if (earlier < index) found.push({ kind: "duplicate", index, of: earlier });
  });
  return found;
}

/** A copy of a built-in theme to start from, under a new name. */
export function startFrom(theme: ThemeSpec, name: string): ThemeSpec {
  return {
    name,
    appearance: theme.appearance,
    colors: { ...resolve(theme).colors },
    radius: theme.radius,
    radius_lg: theme.radius_lg,
    fonts: { ...theme.fonts },
    window_background: theme.window_background,
  };
}

// ---- Preview ----

function toHsl({ r, g, b }: Rgba): [number, number, number] {
  const [x, y, z] = [r / 255, g / 255, b / 255];
  const max = Math.max(x, y, z);
  const min = Math.min(x, y, z);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  const h = max === x ? (y - z) / d + (y < z ? 6 : 0) : max === y ? (z - x) / d + 2 : (x - y) / d + 4;
  return [h / 6, s, l];
}

function fromHsl([h, s, l]: [number, number, number], a: number): Rgba {
  if (s === 0) return { r: l * 255, g: l * 255, b: l * 255, a };
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;
  const hue = (t: number) => {
    const u = t < 0 ? t + 1 : t > 1 ? t - 1 : t;
    if (u < 1 / 6) return p + (q - p) * 6 * u;
    if (u < 1 / 2) return q;
    if (u < 2 / 3) return p + (q - p) * (2 / 3 - u) * 6;
    return p;
  };
  return { r: hue(h + 1 / 3) * 255, g: hue(h) * 255, b: hue(h - 1 / 3) * 255, a };
}

/** A hovered key's shade, as the launcher makes it (theme::nudge): lighter,
 * or darker on near-white. */
export function nudge(text: string): string {
  const color = parseColor(text)!;
  const [h, s, l] = toHsl(color);
  return formatColor(fromHsl([h, s, l > 0.9 ? l - 0.03 : Math.min(l + 0.05, 1)], color.a));
}

const SANS = '"Space Grotesk", "PingFang SC", "Microsoft YaHei UI", "Noto Sans CJK SC", system-ui, sans-serif';
const MONO = '"JetBrains Mono", ui-monospace, "SF Mono", Menlo, monospace';

function family(name: string | undefined, fallback: string): string {
  const own = name?.trim();
  return own ? `"${own.replace(/["\\]/g, "")}", ${fallback}` : fallback;
}

/** The custom properties instrument.css draws with, for this theme. */
export function previewVars(theme: Resolved): Record<string, string> {
  const c = theme.colors;
  const dark = theme.tone === "dark";
  // Chinese text tries the theme's Chinese face before the system's.
  const sans = family(theme.fonts.cjk, SANS);
  return {
    "--page": c.page,
    "--panel": c.panel,
    "--hairline": c.hairline,
    "--ink": c.ink,
    "--muted": c.muted,
    "--key-white": c.key_white,
    "--key-white-hover": nudge(c.key_white),
    "--key-black": c.key_black,
    "--key-black-hover": nudge(c.key_black),
    "--key-grey": c.key_grey,
    "--on-white": c.on_white,
    "--on-black": c.on_black,
    "--orange": c.orange,
    "--orange-hover": nudge(c.orange),
    "--orange-text": c.orange_text,
    "--on-orange": c.on_orange,
    "--danger": c.danger,
    "--danger-text": c.danger_text,
    "--led-off": c.led_off,
    "--ok": c.ok,
    "--display": c.display,
    "--display-ink": c.display_ink,
    "--display-dim": c.display_dim,
    "--display-label": c.display_label,
    "--screw": c.hairline,
    "--radius": `${theme.radius}px`,
    "--radius-lg": `${theme.radiusLg}px`,
    "--key-shadow": dark
      ? "0 1px 0 rgb(0 0 0 / 0.7), 0 3px 6px rgb(0 0 0 / 0.45), inset 0 1px 0 rgb(255 255 255 / 0.16)"
      : "0 1px 0 rgb(0 0 0 / 0.22), 0 3px 6px rgb(0 0 0 / 0.12), inset 0 1px 0 rgb(255 255 255 / 0.7)",
    "--pressed-shadow": "inset 0 1px 2px rgb(0 0 0 / 0.22)",
    "--recess-shadow": `inset 0 1px 2px rgb(0 0 0 / ${dark ? 0.5 : 0.1})`,
    "--display-shadow": "inset 0 1px 3px rgb(0 0 0 / 0.6)",
    "--panel-shadow": `0 4px 12px rgb(0 0 0 / ${dark ? 0.5 : 0.1})`,
    "--font-sans": family(theme.fonts.sans, sans),
    "--font-mono": family(theme.fonts.mono, MONO),
    "--font-display": `"DSEG7 Classic", ${MONO}`,
    "--instant": "90ms",
    "--quick": "160ms",
    "--ease-out-quint": "cubic-bezier(0.22, 1, 0.36, 1)",
    "color-scheme": theme.tone,
  };
}
