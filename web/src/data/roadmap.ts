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
      statusLabel: t.status[item.status as keyof typeof t.status],
      name: translated.name,
      lede: translated.lede,
      source: `${REPO_URL}/blob/main/.agents/plans/${encodeURIComponent(item.planId)}.json`,
    };
  });
}

// The three grey cells in the hero, always the first three upcoming plans.
const upcoming = roadmapItems
  .filter((item) => item.status !== "completed" && item.version)
  .slice(0, 3)
  .map((item) => ({ ...item, version: item.version! }));

if (upcoming.length < 3) throw new Error(`Need at least 3 upcoming roadmap teasers, got ${upcoming.length}`);

export const plannedTeasers = {
  0: upcoming[0],
  1: upcoming[1],
  2: upcoming[2],
} as const;
