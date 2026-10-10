import { useEffect, useState } from "react";
import { AppleLogoIcon as AppleLogo, DownloadSimpleIcon as DownloadSimple, LinuxLogoIcon as LinuxLogo, WindowsLogoIcon as WindowsLogo } from "@phosphor-icons/react";
import { fill, type Catalog } from "../i18n";
import type { PlatformId, Release } from "../data/release";

type Props = {
  t: Catalog["download"];
  release: Release | null;
  releasesUrl: string;
};

const PLATFORMS: { id: PlatformId; Icon: typeof AppleLogo }[] = [
  { id: "macos", Icon: AppleLogo },
  { id: "windows", Icon: WindowsLogo },
  { id: "linux", Icon: LinuxLogo },
];

function detect(): PlatformId {
  const nav = navigator as Navigator & { userAgentData?: { platform?: string } };
  const name = (nav.userAgentData?.platform || navigator.platform || navigator.userAgent).toLowerCase();
  if (name.includes("win")) return "windows";
  if (name.includes("linux") || name.includes("x11")) return "linux";
  return "macos";
}

const megabytes = (bytes: number) => `${(bytes / 1024 / 1024).toFixed(bytes < 10 * 1024 * 1024 ? 1 : 0)} MB`;

/** The download panel: the visitor's platform first, the others one segment
 * key away. Each file goes through the Worker mirror, with GitHub's own link
 * beside it. */
export function Download({ t, release, releasesUrl }: Props) {
  const [platform, setPlatform] = useState<PlatformId>("macos");
  useEffect(() => setPlatform(detect()), []);

  const files = release?.platforms[platform] ?? [];
  const info = t.platforms[platform];

  return (
    <div className="faceplate download__panel">
      <span className="screws" aria-hidden="true" />
      <div className="download__head">
        <div className="segments" role="radiogroup" aria-label={t.title}>
          {PLATFORMS.map(({ id, Icon }) => (
            <div key={id} className="segment">
              <span className="led" data-lit={id === platform} />
              <button
                type="button"
                role="radio"
                aria-checked={id === platform}
                aria-pressed={id === platform}
                className="key"
                onClick={() => setPlatform(id)}
              >
                <Icon weight="fill" aria-hidden="true" />
                {t.platforms[id].name}
              </button>
            </div>
          ))}
        </div>
        <div className="display download__version">
          <span className="display__legend">VERSION</span>
          <span className="digits" data-ghost={(release?.version ?? "0.1.0").replace(/\d/g, "8")}>
            <span>{release?.version ?? "0.1.0"}</span>
          </span>
        </div>
      </div>

      <p className="download__needs muted">{info.needs}</p>

      {release && files.length ? (
        <ul className="download__files">
          {files.map((file, i) => (
            <li key={file.name}>
              <div className="download__file">
                <span>{t.files[file.kind]}</span>
                <span className="mono muted download__name">{file.name}</span>
              </div>
              <span className="mono muted">{megabytes(file.size)}</span>
              <a className="download__github" href={file.github}>
                {t.github}
              </a>
              <a className={`key ${i === 0 ? "key--orange" : ""}`} href={file.mirror}>
                <DownloadSimple weight="bold" aria-hidden="true" />
                {i === 0 ? fill(t.for, { os: info.name }) : t.get}
              </a>
            </li>
          ))}
        </ul>
      ) : (
        <div className="download__pending">
          <p>{t.pending}</p>
          <a className="key" href={releasesUrl}>
            {t.releases}
          </a>
        </div>
      )}

      {release?.checksums && (
        <p className="download__sums">
          <a href={release.checksums.mirror}>{t.checksums}</a>
        </p>
      )}
    </div>
  );
}
