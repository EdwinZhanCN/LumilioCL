// Copies the launcher's own fonts, icons and screenshots into `public/`, so the
// site never keeps a second copy of them in git (`public/{fonts,icons,shots}`
// are ignored). Run by `pnpm dev`, `pnpm build` and `pnpm check`.
import { cpSync, mkdirSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const web = join(dirname(fileURLToPath(import.meta.url)), "..");
const repo = join(web, "..");

const copies = [
  ["crates/lumilio-ui/assets/fonts", "fonts"],
  ["assets/icons/src/tile", "icons"],
  ["assets/screenshots", "shots"],
];

for (const [from, to] of copies) {
  const target = join(web, "public", to);
  rmSync(target, { recursive: true, force: true });
  mkdirSync(target, { recursive: true });
  cpSync(join(repo, from), target, { recursive: true });
}
