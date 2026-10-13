// How the studio groups the launcher's colours for editing. model.test.ts
// checks that the groups cover every colour of themes.generated.json once.

export const COLOR_GROUPS = [
  { id: "body", keys: ["page", "panel", "hairline", "ink", "muted"] },
  { id: "keys", keys: ["key_white", "key_black", "key_grey", "on_white", "on_black"] },
  { id: "signal", keys: ["orange", "orange_text", "on_orange"] },
  { id: "danger", keys: ["danger", "danger_text"] },
  { id: "leds", keys: ["led_off", "ok"] },
  { id: "display", keys: ["display", "display_ink", "display_dim", "display_label"] },
] as const;

export type GroupId = (typeof COLOR_GROUPS)[number]["id"];
