import { readFileSync, readdirSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  BUILTIN_THEMES,
  COLOR_KEYS,
  CONTRAST_RULES,
  MAX_RADIUS,
  checkContrast,
  clampRadius,
  contrast,
  fileName,
  parseColor,
  parseFile,
  problems,
  resolve,
  startFrom,
  writeFile,
  type ThemeSpec,
} from "./model";

const BUNDLED = new URL("../../../../crates/lumilio-ui/themes/", import.meta.url);

describe("colours", () => {
  it("reads what the launcher reads (theme/tests colours_may_carry_opacity)", () => {
    expect(parseColor("#ff5a1a")).toEqual({ r: 255, g: 90, b: 26, a: 255 });
    expect(parseColor("ff5a1a")?.a).toBe(255);
    expect(parseColor("#ff5a1a80")?.a).toBe(128);
    for (const bad of ["orange", "#12345", "#ggg", "", "#ff5a1a8", "#é5a1a"]) {
      expect(parseColor(bad), bad).toBeNull();
    }
  });

  it("measures contrast as WCAG 2 does", () => {
    expect(contrast(parseColor("#000000")!, parseColor("#ffffff")!)).toBeCloseTo(21, 5);
    expect(contrast(parseColor("#777777")!, parseColor("#777777")!)).toBe(1);
  });
});

describe("theme files", () => {
  it("reads every theme file the launcher ships", () => {
    const files = readdirSync(BUNDLED).filter((name) => name.endsWith(".json"));
    expect(files.length).toBeGreaterThan(0);
    for (const file of files) {
      const parsed = parseFile(readFileSync(new URL(file, BUNDLED), "utf8"));
      expect(parsed.ok, file).toBe(true);
      if (!parsed.ok) continue;
      for (const theme of parsed.themes) {
        expect(theme.ok, `${file}: ${theme.ok ? "" : theme.error}`).toBe(true);
        if (theme.ok) expect(resolve(theme.theme).missing).toEqual([]);
      }
    }
  });

  it("writes every built-in theme so that it reads back unchanged", () => {
    const written = writeFile(BUILTIN_THEMES);
    const parsed = parseFile(written);
    expect(parsed.ok).toBe(true);
    if (!parsed.ok) return;
    parsed.themes.forEach((theme, index) => {
      expect(theme.ok).toBe(true);
      if (theme.ok) expect(resolve(theme.theme).colors).toEqual(resolve(BUILTIN_THEMES[index]).colors);
    });
    expect(JSON.parse(written).themes[0]).not.toHaveProperty("window_background");
  });

  it("keeps one bad theme from hiding the others in its file", () => {
    const parsed = parseFile(
      JSON.stringify({
        themes: [
          { name: "Broken", appearance: "dark", colors: { page: "red" } },
          { name: "Fine", appearance: "light", colors: { ink: "#101010" }, extra: true },
        ],
      }),
    );
    expect(parsed.ok).toBe(true);
    if (!parsed.ok) return;
    expect(parsed.themes[0]).toMatchObject({ ok: false });
    expect(parsed.themes[0].ok ? "" : parsed.themes[0].error).toMatch(/^Broken: `red` is not a colour/);
    expect(parsed.themes[1]).toMatchObject({ ok: true, theme: { name: "Fine", colors: { ink: "#101010" } } });
    expect(parseFile("{}")).toEqual({ ok: false, error: "missing field `themes`" });
    expect(parseFile("not json").ok).toBe(false);
  });

  it("fills what a theme leaves out from the built-in theme of its tone", () => {
    const theme: ThemeSpec = { name: "Sparse", appearance: "dark", colors: { page: "#000000" }, fonts: {} };
    const resolved = resolve(theme);
    const night = BUILTIN_THEMES.find((builtin) => builtin.name === "Night")!;
    expect(resolved.colors.page).toBe("#000000");
    expect(resolved.colors.ink).toBe(night.colors.ink);
    expect(resolved.missing).toEqual(COLOR_KEYS.filter((key) => key !== "page"));
  });

  it("lets only the body be translucent, as the launcher does", () => {
    const theme: ThemeSpec = {
      name: "Glass",
      appearance: "light",
      colors: { page: "#ffffff80", orange: "#f0520f80" },
      fonts: {},
    };
    const resolved = resolve(theme);
    expect(resolved.colors.page).toBe("#ffffff80");
    expect(resolved.colors.orange).toBe("#f0520f");
    expect(resolved.solid).toEqual(["orange"]);
  });

  it("holds radii where the launcher holds them", () => {
    expect(clampRadius(40, 4)).toBe(MAX_RADIUS);
    expect(clampRadius(-2, 4)).toBe(0);
    expect(clampRadius(Number.NaN, 4)).toBe(4);
    expect(clampRadius(undefined, 6)).toBe(6);
  });

  it("names the download after the first theme", () => {
    const theme = startFrom(BUILTIN_THEMES[0], "Rosé Pine Mist");
    expect(fileName([theme])).toBe("rose-pine-mist.json");
    expect(fileName([{ ...theme, name: "灰" }])).toBe("theme.json");
  });
});

describe("problems the launcher would have", () => {
  it("finds empty names, built-in names and repeated themes", () => {
    const base = startFrom(BUILTIN_THEMES[0], "Mine");
    const themes: ThemeSpec[] = [
      base,
      { ...base, name: "" },
      { ...base, name: "Aluminium", appearance: "light" },
      { ...base },
      { ...base, appearance: "dark" },
    ];
    expect(problems(themes)).toEqual([
      { kind: "no-name", index: 1 },
      { kind: "builtin", index: 2 },
      { kind: "duplicate", index: 3, of: 0 },
    ]);
  });
});

describe("contrast rules", () => {
  it("passes the launcher's own themes, as the Rust test does", () => {
    for (const name of ["Aluminium", "Night"]) {
      const theme = BUILTIN_THEMES.find((builtin) => builtin.name === name)!;
      const failed = checkContrast(resolve(theme)).filter((result) => !result.ok);
      expect(failed.map((result) => result.rule.id), name).toEqual([]);
    }
  });

  it("flags text that does not stand out", () => {
    const grey: ThemeSpec = { name: "Grey", appearance: "light", colors: { ink: "#d0d0d0" }, fonts: {} };
    const failed = checkContrast(resolve(grey)).filter((result) => !result.ok);
    expect(failed.map((result) => result.rule.id)).toContain("ink-page");
  });
});

describe("studio text and grouping", () => {
  it("groups every colour once and names every colour and rule in both languages", async () => {
    const { COLOR_GROUPS } = await import("./groups");
    const { studioText } = await import("../../i18n/studio");
    const grouped = COLOR_GROUPS.flatMap((group) => [...group.keys]);
    expect([...grouped].sort()).toEqual([...COLOR_KEYS].sort());
    expect(new Set(grouped).size).toBe(grouped.length);
    for (const language of ["zh-CN", "en"]) {
      const t = studioText(language);
      for (const key of COLOR_KEYS) expect(t.colours[key], `${language} ${key}`).toBeTruthy();
      for (const group of COLOR_GROUPS) expect(t.groups[group.id], `${language} ${group.id}`).toBeTruthy();
      for (const rule of CONTRAST_RULES) expect(t.rules[rule.id], `${language} ${rule.id}`).toBeTruthy();
    }
  });
});
