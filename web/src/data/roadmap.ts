import generated from "./roadmap.generated.json";

export const roadmapItems = generated.items;

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
