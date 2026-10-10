import { useEffect, useRef, useState } from "react";
import { motion, useMotionValueEvent, useReducedMotion, useScroll, useTransform } from "motion/react";
import { fill, type Catalog } from "../i18n";
import { useMounted } from "../lib/useMounted";

type Props = { t: Catalog["audiences"] };

function useWide() {
  const [wide, setWide] = useState(true);
  useEffect(() => {
    const query = matchMedia("(min-width: 900px)");
    const update = () => setWide(query.matches);
    update();
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, []);
  return wide;
}

/** Three readers, three panels. On a wide screen the section pins and
 * vertical scrolling pans sideways, pausing on each panel; on a narrow
 * screen, with reduced motion, or without JS it is a row you swipe. The
 * segment keys above follow the panel in view and jump to the others. */
export function Audiences({ t }: Props) {
  const reduce = useReducedMotion() ?? false;
  const wide = useWide();
  const pinned = useMounted() && wide && !reduce;
  const section = useRef<HTMLElement>(null);
  const track = useRef<HTMLDivElement>(null);
  const [distance, setDistance] = useState(0);
  const [hold, setHold] = useState(0);
  const [current, setCurrent] = useState(0);
  const { scrollYProgress } = useScroll({ target: section, offset: ["start start", "end end"] });
  const count = t.groups.length;

  // Each panel holds still for `hold` px of scrolling before the track moves
  // on, so the pan pauses on every panel instead of sweeping past them all.
  const seg = distance / Math.max(1, count - 1);
  const length = distance + count * hold;
  const stop = (i: number) => i * (hold + seg);
  // Until measured, keyframes would all sit at 0; hold the track still.
  const input: number[] = length ? [] : [0, 1];
  const output: number[] = length ? [] : [0, 0];
  if (length)
    for (let i = 0; i < count; i++) {
      input.push(stop(i) / length, (stop(i) + hold) / length);
      output.push(-i * seg, -i * seg);
    }
  const x = useTransform(scrollYProgress, input, output);

  useMotionValueEvent(scrollYProgress, "change", (p) => {
    if (!pinned || !length) return;
    // The panel whose hold is nearest the scroll position.
    const at = p * length;
    let nearest = 0;
    for (let i = 1; i < count; i++)
      if (Math.abs(at - stop(i) - hold / 2) < Math.abs(at - stop(nearest) - hold / 2)) nearest = i;
    setCurrent(nearest);
  });

  useEffect(() => {
    const element = track.current;
    if (!element || !pinned) return;
    const measure = () => {
      setDistance(Math.max(0, element.scrollWidth - window.innerWidth));
      setHold(Math.round(window.innerHeight * 0.6));
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    window.addEventListener("resize", measure);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, [pinned]);

  // Swiped row: the panel mostly in view is the current one.
  useEffect(() => {
    const element = track.current;
    if (!element || pinned) return;
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries)
          if (entry.isIntersecting) setCurrent(Number((entry.target as HTMLElement).dataset.index));
      },
      { root: element, threshold: 0.6 },
    );
    element.querySelectorAll("[data-index]").forEach((panel) => observer.observe(panel));
    return () => observer.disconnect();
  }, [pinned]);

  const go = (i: number) => {
    if (pinned && section.current) {
      const top = section.current.getBoundingClientRect().top + window.scrollY;
      window.scrollTo({ top: top + stop(i) + hold / 2, behavior: reduce ? "auto" : "smooth" });
    } else {
      track.current?.querySelector<HTMLElement>(`[data-index="${i}"]`)?.scrollIntoView({
        behavior: reduce ? "auto" : "smooth",
        inline: "start",
        block: "nearest",
      });
    }
  };

  return (
    <section
      ref={section}
      id="audiences"
      className={`audiences ${pinned ? "audiences--pinned" : ""}`}
      style={pinned ? { height: `calc(100dvh + ${length}px)` } : undefined}
      aria-labelledby="audiences-title"
    >
      {/* A gentle snap point in the middle of each panel's hold. */}
      {pinned &&
        t.groups.map((group, i) => (
          <span key={group.id} className="audiences__snap" style={{ top: stop(i) + hold / 2 }} aria-hidden="true" />
        ))}
      <div className="audiences__stage">
        <header className="column audiences__head">
          <div className="section-head">
            <h2 id="audiences-title" className="title">
              {t.title}
            </h2>
            <p className="lede">{t.lede}</p>
          </div>
          <div className="segments" role="tablist" aria-label={t.title}>
            {t.groups.map((group, i) => (
              <div key={group.id} className="segment">
                <span className="led" data-lit={i === current} />
                <button
                  type="button"
                  role="tab"
                  aria-selected={i === current}
                  aria-pressed={i === current}
                  aria-controls={`audience-${group.id}`}
                  className="key"
                  onClick={() => go(i)}
                >
                  {group.name}
                </button>
              </div>
            ))}
          </div>
        </header>
        <motion.div ref={track} className="audiences__track" style={pinned ? { x } : undefined}>
          {t.groups.map((group, i) => (
            <AudiencePanel key={group.id} group={group} index={i} shotLabel={t.shot} />
          ))}
        </motion.div>
      </div>
    </section>
  );
}

type Group = Props["t"]["groups"][number];

/** One reader's panel. Each point is a key: choosing it shows that point's
 * picture, so the list reads as a set of things to look at. */
function AudiencePanel({ group, index, shotLabel }: { group: Group; index: number; shotLabel: string }) {
  const [chosen, setChosen] = useState(0);
  const point = group.points[chosen];
  // A point without its own picture yet shows the group's.
  const shot: string | null = point.shot ?? group.shot;
  const label = point.shot ? point.text : fill(shotLabel, { name: group.shotName });

  return (
    <article
      id={`audience-${group.id}`}
      data-index={index}
      role="tabpanel"
      className="faceplate audience"
      aria-label={group.name}
    >
      <span className="screws" aria-hidden="true" />
      <div className="audience__text">
        <span className="tag tag--ink">{group.name}</span>
        <h3 className="audience__title">{group.title}</h3>
        <p className="lede">{group.lede}</p>
        <ul className="audience__points" aria-label={group.title}>
          {group.points.map((item, i) => (
            <li key={item.text}>
              <button
                type="button"
                className="audience__point"
                aria-pressed={i === chosen}
                aria-controls={`audience-${group.id}-shot`}
                onClick={() => setChosen(i)}
              >
                <span className="led" data-lit={i === chosen} />
                {item.text}
              </button>
            </li>
          ))}
        </ul>
      </div>
      <figure id={`audience-${group.id}-shot`} className="window audience__shot" aria-live="polite">
        {shot ? (
          <img key={shot} src={shot} alt={label} loading="lazy" decoding="async" />
        ) : (
          // TODO(asset): this point's screenshot, 16:10.
          <div className="placeholder" role="img" aria-label={point.text}>
            <span className="silk">{point.text}</span>
          </div>
        )}
      </figure>
    </article>
  );
}
