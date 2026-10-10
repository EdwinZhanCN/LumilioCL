import { describe, expect, it } from "vitest";
import { plannedTeasers, roadmapItems } from "./roadmap";

describe("public plan projection", () => {
  it("keeps the three curated logo cells linked to stable public plan IDs", () => {
    expect(plannedTeasers.appearance.planId).toBe("appearance");
    expect(plannedTeasers.translation.planId).toBe("discover-translation");
    expect(plannedTeasers.plugins.planId).toBe("wasm-plugin-registry");
    for (const teaser of Object.values(plannedTeasers)) {
      expect(roadmapItems.some((item) => item.id === teaser.id)).toBe(true);
      expect(teaser.version).toBeTruthy();
    }
  });
});
