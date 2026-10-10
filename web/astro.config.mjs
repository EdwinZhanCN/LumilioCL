import { defineConfig } from "astro/config";
import react from "@astrojs/react";

export default defineConfig({
  site: "https://launcher.lumilio.org",
  integrations: [react()],
  devToolbar: { enabled: false },
  // Chinese is the source language and lives at the root; English joins at
  // /en/ once its catalog is written (src/i18n).
  i18n: {
    locales: ["zh-cn", "en"],
    defaultLocale: "zh-cn",
    routing: { prefixDefaultLocale: false },
  },
});
