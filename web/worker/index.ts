// launcher.lumilio.org: the site's static files, plus the release mirror.
// wrangler.jsonc sends only /releases/* here (`run_worker_first`); every
// other path is served from the built site without running this code.

import { proxy } from "./proxy";

type Env = { ASSETS: { fetch(request: Request): Promise<Response> } };

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const { pathname } = new URL(request.url);
    if (pathname.startsWith("/releases/")) return proxy(request);
    return env.ASSETS.fetch(request);
  },
};
