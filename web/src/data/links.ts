export const REPO = "EdwinZhanCN/LumilioCL";
export const REPO_URL = `https://github.com/${REPO}`;
export const RELEASES_URL = `${REPO_URL}/releases`;
export const LICENSE_URL = `${REPO_URL}/blob/main/LICENSE`;
export const ATTRIBUTIONS_URL = `${REPO_URL}/blob/main/ATTRIBUTIONS.md`;
export const UNSIGNED_HELP_URL = `${REPO_URL}#第一次打开时系统弹出警告`;

/** The Worker's mirror of GitHub release downloads (web/worker). */
export const MIRROR = "https://launcher.lumilio.org";

/** Where each technology the stack section names is documented, keyed by its
 * legend (UI / GPU / CORE / DATA). */
export const TECH_URLS: Record<string, string> = {
  UI: "https://gpui-kit.com",
  GPU: "https://wgpu.rs",
  CORE: "https://doc.rust-lang.org/book/",
  DATA: "https://sqlite.org",
};
