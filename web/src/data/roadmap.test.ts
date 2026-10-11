import { describe, expect, it } from "vitest";
import { plannedTeasers, primaryPlans, roadmapItems } from "./roadmap";

describe("public plan projection", () => {
  it("keeps completed milestones completed after a newer patch release", () => {
    const chinese = primaryPlans("zh-CN");
    const english = primaryPlans("en");
    for (const version of ["0.1.0", "0.1.1"]) {
      expect(chinese.find((plan) => plan.version === version)?.statusLabel).toBe("已完成");
      expect(english.find((plan) => plan.version === version)?.statusLabel).toBe("Completed");
    }
    expect(chinese.find((plan) => plan.version === "0.4.0")?.statusLabel).toBe("提议中");
  });
  it("keeps primary selection, lifecycle and target versions identical in both languages", () => {
    const chinese = primaryPlans("zh-CN");
    const english = primaryPlans("en");
    for (const plans of [chinese, english]) {
      expect(plans.map(({ planId, status, version }) => ({ planId, status, version })))
        .toEqual(roadmapItems.map(({ planId, status, version }) => ({ planId, status, version })));
      expect(plans.every((plan) => plan.source.endsWith(`${plan.planId}.json`))).toBe(true);
    }
    expect(english.find((plan) => plan.planId === "appearance")?.name).toBe("Themes and appearance");
  });
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
