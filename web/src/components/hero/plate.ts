// What each of the logo's nine cells means (assets/icons/src/tile/tile-light.svg):
// the five dark cells are what the launcher does today, the three light cells
// are the roadmap, and the orange circle is the one primary key.

import { makeScene, type Scene, type SceneId } from "../../world/scenes";
import type { Catalog } from "../../i18n";

export type FeatureId = keyof Catalog["features"];
export type PlannedId = keyof Catalog["planned"];

export type Cell =
  | { kind: "feature"; id: FeatureId; scene: SceneId; index: number; shot: string | null }
  | { kind: "planned"; id: PlannedId }
  | { kind: "launch" };

// Row-major, as in the icon.
export const CELLS: readonly Cell[] = [
  { kind: "feature", id: "library", scene: "dawn", index: 1, shot: "/shots/game-library.png" },
  { kind: "planned", id: "appearance" },
  { kind: "launch" },
  { kind: "feature", id: "discover", scene: "portal", index: 2, shot: "/shots/discovery.png" },
  { kind: "planned", id: "translation" },
  { kind: "planned", id: "plugins" },
  // TODO(asset): screenshots or GIFs for these three, described in the catalog's `shot`.
  { kind: "feature", id: "map", scene: "map", index: 3, shot: null },
  { kind: "feature", id: "schematic", scene: "redstone", index: 4, shot: null },
  { kind: "feature", id: "diagnostics", scene: "caves", index: 5, shot: null },
];

export const FEATURE_COUNT = CELLS.filter((cell) => cell.kind === "feature").length;

// Stable scene factories, so canvases are not rebuilt on every render.
export const makers: Record<SceneId, (w: number, h: number) => Scene> = {
  dawn: (w, h) => makeScene("dawn", w, h),
  portal: (w, h) => makeScene("portal", w, h),
  map: (w, h) => makeScene("map", w, h),
  redstone: (w, h) => makeScene("redstone", w, h),
  caves: (w, h) => makeScene("caves", w, h),
};

export const pad = (n: number) => String(n).padStart(2, "0");
