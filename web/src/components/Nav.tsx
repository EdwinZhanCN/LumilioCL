import { useEffect, useState } from "react";
import { MotionConfig, motion } from "motion/react";
import {
  CpuIcon as Cpu,
  DownloadSimpleIcon as DownloadSimple,
  GaugeIcon as Gauge,
  GithubLogoIcon as GithubLogo,
  HouseIcon as House,
  PathIcon as Path,
  UsersThreeIcon as UsersThree,
} from "@phosphor-icons/react";
import type { Catalog } from "../i18n";

type Props = { t: Catalog["nav"]; repo: string };

const LANDMARKS = [
  { id: "home", Icon: House },
  { id: "proof", Icon: Gauge },
  { id: "audiences", Icon: UsersThree },
  { id: "stack", Icon: Cpu },
  { id: "roadmap", Icon: Path },
  { id: "download", Icon: DownloadSimple },
] as const;

type LandmarkId = (typeof LANDMARKS)[number]["id"];

/** The launcher's bottom navigation (design language §6, ADR 0012): the
 * current location on the leading side, the landmark capsule centred, the
 * source on the trailing side. The open landmark sits pressed with its LED
 * lit and opens out to show its label. */
export function Nav({ t, repo }: Props) {
  const [active, setActive] = useState<LandmarkId>("home");

  useEffect(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) if (entry.isIntersecting) setActive(entry.target.id as LandmarkId);
      },
      { rootMargin: "-45% 0px -45% 0px" },
    );
    for (const { id } of LANDMARKS) {
      const section = document.getElementById(id);
      if (section) observer.observe(section);
    }
    return () => observer.disconnect();
  }, []);

  return (
    <MotionConfig reducedMotion="user">
      <nav className="nav" aria-label={t.label}>
        <div className="nav__zone nav__lead">
          <img className="on-light" src="/icons/tile-light.svg" alt="" width="20" height="20" />
          <img className="on-dark" src="/icons/tile-dark.svg" alt="" width="20" height="20" />
          <span>{t[active]}</span>
        </div>
        <div className="nav__zone nav__capsule">
          {LANDMARKS.map(({ id, Icon }) => {
            const open = id === active;
            return (
              <motion.a
                key={id}
                layout
                transition={{ type: "spring", stiffness: 520, damping: 38 }}
                href={`#${id}`}
                className="key key--small nav__key"
                aria-current={open ? "location" : undefined}
                aria-pressed={open}
                title={t[id]}
                aria-label={t[id]}
              >
                <span className="led" data-lit={open} />
                <Icon weight={open ? "fill" : "regular"} aria-hidden="true" />
                {open && (
                  <motion.span layout="position" className="nav__label" initial={{ opacity: 0 }} animate={{ opacity: 1 }}>
                    {t[id]}
                  </motion.span>
                )}
              </motion.a>
            );
          })}
        </div>
        <div className="nav__zone nav__trail">
          <a className="key key--black key--icon" href={repo} title={t.source} aria-label={t.source}>
            <GithubLogo weight="bold" aria-hidden="true" />
          </a>
        </div>
      </nav>
    </MotionConfig>
  );
}
