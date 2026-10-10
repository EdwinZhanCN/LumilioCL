import { zhCN, type Catalog } from "./zh-cn";

// English joins here (and in astro.config.mjs `i18n.locales` routing) once
// `en.ts` is written against `Catalog`.
const catalogs: Record<string, Catalog> = {
  "zh-cn": zhCN,
};

export type { Catalog };

export function catalog(locale: string | undefined): Catalog {
  return catalogs[locale ?? "zh-cn"] ?? zhCN;
}

export function fill(template: string, values: Record<string, string>): string {
  return template.replace(/\{(\w+)\}/g, (_, key: string) => values[key] ?? "");
}
