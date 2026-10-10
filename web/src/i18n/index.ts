import { zhCN, type Catalog } from "./zh-cn";

// Marketing catalogs are separate from Starlight's document language.
// Add `en.ts` here when the English marketing catalog is available.
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
