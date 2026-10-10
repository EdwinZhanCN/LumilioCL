import { useEffect, useRef, useState } from "react";
import { ArrowUpRightIcon as ArrowUpRight } from "@phosphor-icons/react";
import { motion, useMotionValueEvent, useReducedMotion, useScroll, useTransform, type MotionValue } from "motion/react";
import { TECH_URLS } from "../data/links";
import { fill, type Catalog } from "../i18n";
import { useMounted } from "../lib/useMounted";

type Props = { t: Catalog["stack"] };

// The lit LED travels to the next row instead of blinking off there and on
// here (ease_out_quint, design language §2); reduced motion keeps it still.
const LED = { duration: 0.34, ease: [0.22, 1, 0.36, 1] } as const;

/** Wide enough to pin: below this the grid folds into one column, which will
 * not fit a viewport. Matches the Audiences breakpoint. */
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

/** The launcher's layers as an exploded drawing. On a wide screen the section
 * pins and scrolling steps the plates apart one layer at a time, pausing on
 * each; the row for the layer in front lights up. On a narrow screen, with
 * reduced motion, or without JS the plates spread as the section scrolls by. */
export function Stack({ t }: Props) {
  const reduce = useReducedMotion() ?? false;
  const mounted = useMounted();
  const wide = useWide();
  const pinned = mounted && wide && !reduce;
  const still = !mounted || reduce;

  const section = useRef<HTMLElement>(null);
  const [active, setActive] = useState(0);
  const [hold, setHold] = useState(0);
  const [span, setSpan] = useState(0);
  const count = t.layers.length;

  const { scrollYProgress } = useScroll({ target: section, offset: ["start start", "end end"] });

  // Each layer holds still for `hold` px of scrolling before the stack opens on
  // to the next, so the drawing pauses on every layer instead of sweeping past
  // them all (same mechanic as the Audiences pan).
  const seg = span;
  const length = count * hold + Math.max(0, count - 1) * seg;
  const stop = (i: number) => i * (hold + seg);

  const openKeys: number[] = [];
  const openValues: number[] = [];
  if (length) {
    for (let i = 0; i < count; i++) {
      const step = count > 1 ? i / (count - 1) : 0;
      openKeys.push(stop(i) / length, (stop(i) + hold) / length);
      openValues.push(step, step);
    }
  }
  // Until measured (unpinned, or before the first effect), fall back to the
  // plain scroll spread so nothing jumps.
  const open = useTransform(scrollYProgress, length ? openKeys : [0, 0.75], length ? openValues : [0, 1]);

  useMotionValueEvent(scrollYProgress, "change", (p) => {
    if (!length) {
      setActive(Math.min(count - 1, Math.floor(p * count)));
      return;
    }
    // The layer whose hold is nearest the scroll position.
    const at = p * length;
    let nearest = 0;
    for (let i = 1; i < count; i++)
      if (Math.abs(at - stop(i) - hold / 2) < Math.abs(at - stop(nearest) - hold / 2)) nearest = i;
    setActive(nearest);
  });

  useEffect(() => {
    if (!pinned) return;
    const measure = () => {
      const h = window.innerHeight;
      setHold(Math.round(h * 0.6));
      setSpan(Math.round(h * 0.4));
    };
    measure();
    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
  }, [pinned]);

  return (
    <section
      ref={section}
      id="stack"
      className={`stack ${pinned ? "stack--pinned" : ""}`}
      style={pinned ? { height: `calc(100dvh + ${length}px)` } : undefined}
      aria-labelledby="stack-title"
    >
      {/* A gentle snap point in the middle of each layer's hold. */}
      {pinned &&
        t.layers.map((layer, i) => (
          <span key={layer.legend} className="stack__snap" style={{ top: stop(i) + hold / 2 }} aria-hidden="true" />
        ))}
      <div className="stack__stage">
        <div className="column stack__grid">
          <header className="section-head stack__head">
            <h2 id="stack-title" className="title">
              {t.title}
            </h2>
            <p className="lede">{t.lede}</p>
          </header>
          <div className="stack__drawing" aria-hidden="true">
            <div className="stack__iso">
              {t.layers.map((layer, i) => (
                <Plate
                  key={layer.legend}
                  index={i}
                  count={count}
                  open={open}
                  still={still}
                  lit={still || i === active}
                  legend={layer.legend}
                />
              ))}
            </div>
          </div>
          <table className="table stack__table">
            <tbody>
              {t.layers.map((layer, i) => {
                const href = TECH_URLS[layer.legend];
                const site = href ? new URL(href).host : "";
                return (
                  <tr key={layer.legend} data-active={still || i === active}>
                    <th scope="row">
                      <span className="stack__label">
                        {still ? (
                          <span className="led" data-lit="true" />
                        ) : i === active ? (
                          <motion.span layoutId="stack-led" className="led" data-lit="true" transition={LED} />
                        ) : (
                          <span className="led" data-lit="false" />
                        )}
                        {layer.name}
                      </span>
                    </th>
                    <td>
                      <span className="stack__tech">{layer.tech}</span>
                      <span className="stack__note">{layer.note}</span>
                    </td>
                    <td className="stack__link">
                      {href && (
                        <a
                          className="key key--icon"
                          href={href}
                          target="_blank"
                          rel="noopener noreferrer"
                          aria-label={fill(t.link, { site })}
                          title={site}
                        >
                          <ArrowUpRight weight="bold" aria-hidden="true" />
                        </a>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    </section>
  );
}

// The top plate (UI) travels farthest, the bottom one (DATA) not at all:
// nearer layers move faster, as in any parallax.
function Plate({
  index,
  count,
  open,
  still,
  lit,
  legend,
}: {
  index: number;
  count: number;
  open: MotionValue<number>;
  still: boolean;
  lit: boolean;
  legend: string;
}) {
  const depth = count - 1 - index;
  const spread = useTransform(open, [0, 1], [depth * 14, depth * 64]);
  return (
    <motion.div
      className={`stack__plate stack__plate--${legend.toLowerCase()}`}
      data-lit={lit}
      style={{ z: still ? depth * 50 : spread, zIndex: depth }}
    >
      <span className="stack__legend">{legend}</span>
    </motion.div>
  );
}
