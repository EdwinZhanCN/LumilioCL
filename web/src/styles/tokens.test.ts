import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { BUILTIN_THEMES, normalizeColor } from "../components/studio/model";

// tokens.css repeats the launcher's colours for the marketing page; this
// keeps the copy equal to Aluminium and Night (themes.generated.json).
const css = readFileSync(new URL("./tokens.css", import.meta.url), "utf8");

function declarations(block: string): Record<string, string> {
  const found: Record<string, string> = {};
  for (const [, name, value] of block.matchAll(/--([a-z-]+):\s*(#[0-9a-fA-F]{6,8})\s*;/g)) found[name] = value;
  return found;
}

const light = declarations(css.slice(css.indexOf(":root {"), css.indexOf("@media (prefers-color-scheme: dark)")));
const dark = {
  ...light,
  ...declarations(css.slice(css.indexOf("@media (prefers-color-scheme: dark)"), css.indexOf("@media (prefers-reduced-motion"))),
};

describe("tokens.css", () => {
  for (const [name, tokens] of [["Aluminium", light], ["Night", dark]] as const) {
    it(`matches ${name}`, () => {
      const theme = BUILTIN_THEMES.find((builtin) => builtin.name === name)!;
      let compared = 0;
      for (const [key, colour] of Object.entries(theme.colors)) {
        const token = tokens[key.replaceAll("_", "-")];
        if (token === undefined) continue;
        expect(normalizeColor(token), `--${key.replaceAll("_", "-")}`).toBe(colour);
        compared += 1;
      }
      expect(compared).toBeGreaterThan(10);
    });
  }
});
