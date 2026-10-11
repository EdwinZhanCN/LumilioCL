import generated from "./roadmap.generated.json";
import { docsCatalog, type DocsLanguage } from "../i18n/docs";
import { REPO_URL } from "./links";

export const roadmapItems = generated.items;

// The generator selects primary plans: public + roadmap.enabled, excluding
// cancelled plans. Localized copy cannot change status, order or target version.
export function primaryPlans(language: DocsLanguage) {
  const t = docsCatalog(language).roadmap;
  return roadmapItems.map((item) => {
    const translated = language === "en" ? t.plans[item.planId] : item;
    if (!translated) throw new Error(`Missing English primary plan copy: ${item.planId}`);
    return {
      ...item,
      name: translated.name,
      lede: translated.lede,
      source: `${REPO_URL}/blob/main/.agents/plans/${encodeURIComponent(item.planId)}.json`,
    };
  });
}

// These three IDs are curated positions in the logo, independent of list order.
function teaser(id: string) {
  const item = roadmapItems.find((entry) => entry.id === id);
  if (!item || !item.version) throw new Error(`Missing public roadmap teaser: ${id}`);
  return { ...item, version: item.version };
}

export const plannedTeasers = {
  appearance: teaser("appearance"),
  translation: teaser("discover-translation"),
  plugins: teaser("wasm-plugin-registry"),
};
