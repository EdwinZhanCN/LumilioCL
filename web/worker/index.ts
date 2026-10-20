// launcher.lumilio.org: the site's static files, plus the release mirror,
// the block-asset bundle and Discover's summary translations. wrangler.jsonc
// sends only /releases/*, /block-assets/* and /translate/* here
// (`run_worker_first`); every other path is served from the built site
// without running this code.

import { proxy } from "./proxy";
import { type TranslateEnv, translate } from "./translate";

type Env = TranslateEnv & { ASSETS: { fetch(request: Request): Promise<Response> } };

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const { pathname } = new URL(request.url);
    if (pathname.startsWith("/releases/") || pathname.startsWith("/block-assets/")) return proxy(request);
    if (pathname.startsWith("/translate/")) return translate(request, env);
    return env.ASSETS.fetch(request);
  },
};
