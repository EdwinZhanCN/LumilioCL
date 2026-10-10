import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, MotionConfig, motion, useReducedMotion } from "motion/react";
import { ArrowDownIcon as ArrowDown } from "@phosphor-icons/react";
import type { Catalog } from "../../i18n";
import { PixelCanvas } from "../../world/PixelCanvas";
import { CELLS, FEATURE_COUNT, makers, pad, type Cell } from "./plate";
import { CLOSE, Explorer } from "./Explorer";

type Props = { t: Pick<Catalog, "hero" | "features" | "planned" | "launch"> };

type Openable = Exclude<Cell, { kind: "launch" }>;

const STAGE_MS = 420;

/** The hero: the app icon as a working control panel. Hovering a dark cell
 * loads its world chunk by chunk; pressing it opens the feature; the light
 * cells read out the roadmap; the orange key runs the launch moment and
 * lands on the downloads. */
export function Faceplate({ t }: Props) {
  const reduce = useReducedMotion() ?? false;
  const [hover, setHover] = useState<number | null>(null);
  const [open, setOpen] = useState<number | null>(null);
  const [stage, setStage] = useState(-1);
  const keys = useRef<(HTMLButtonElement | null)[]>([]);
  const timers = useRef<number[]>([]);
  const [portal, setPortal] = useState<HTMLElement | null>(null);

  useEffect(() => {
    setPortal(document.body);
    return () => timers.current.forEach(clearTimeout);
  }, []);

  const close = useCallback(() => {
    setOpen((cell) => {
      if (cell !== null) requestAnimationFrame(() => keys.current[cell]?.focus({ preventScroll: true }));
      return null;
    });
  }, []);

  const launch = () => {
    const target = document.getElementById("download");
    if (reduce) {
      target?.scrollIntoView();
      return;
    }
    if (stage >= 0) return;
    timers.current.forEach(clearTimeout);
    setStage(0);
    const after = (ms: number, fn: () => void) => timers.current.push(window.setTimeout(fn, ms));
    for (let s = 1; s < t.launch.stages.length; s++) after(STAGE_MS * s, () => setStage(s));
    after(STAGE_MS * t.launch.stages.length + 260, () => target?.scrollIntoView({ behavior: "smooth" }));
    after(STAGE_MS * t.launch.stages.length + 1400, () => setStage(-1));
  };

  // Arrow keys move between cells like a keypad.
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const index = keys.current.findIndex((key) => key === document.activeElement);
    if (index < 0) return;
    const moves: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -3, ArrowDown: 3 };
    const move = moves[event.key];
    if (move === undefined) return;
    const next = index + move;
    if (next < 0 || next > 8 || (Math.abs(move) === 1 && Math.floor(next / 3) !== Math.floor(index / 3))) return;
    event.preventDefault();
    keys.current[next]?.focus();
  };

  // During the launch moment the worlds load in step with the stages.
  const loadedDuringLaunch = stage < 0 ? 0 : Math.ceil(((stage + 1) * FEATURE_COUNT) / t.launch.stages.length);
  const openCell = open === null ? null : (CELLS[open] as Openable);

  return (
    <MotionConfig reducedMotion="user">
      <div className="plate-wrap">
        <div className="plate" role="group" aria-label={t.hero.plateLabel} onKeyDown={onKeyDown}>
          {["tl", "tr", "bl", "br"].map((corner) => (
            <span key={corner} className={`plate__screw plate__screw--${corner}`} aria-hidden="true" />
          ))}
          <div className="plate__grid">
            {CELLS.map((cell, i) => {
              const bind = {
                ref: (el: HTMLButtonElement | null) => {
                  keys.current[i] = el;
                },
                onPointerEnter: () => setHover(i),
                onPointerLeave: () => setHover((h) => (h === i ? null : h)),
                onFocus: () => setHover(i),
                onBlur: () => setHover((h) => (h === i ? null : h)),
              };
              if (cell.kind === "launch")
                return (
                  <button
                    key="launch"
                    type="button"
                    className="cell cell--launch"
                    aria-label={t.hero.download}
                    aria-busy={stage >= 0}
                    data-held={stage >= 0}
                    onClick={launch}
                    {...bind}
                  >
                    <ArrowDown weight="bold" className="cell__arrow" aria-hidden="true" />
                  </button>
                );
              const isOpen = open === i;
              const name = cell.kind === "feature" ? t.features[cell.id].name : t.planned[cell.id].name;
              const featureOrder = cell.kind === "feature" ? cell.index - 1 : -1;
              const shown = hover === i || (cell.kind === "feature" && featureOrder < loadedDuringLaunch);
              return (
                <button
                  key={cell.id}
                  type="button"
                  className={`cell cell--${cell.kind}`}
                  aria-label={name}
                  aria-haspopup="dialog"
                  aria-expanded={isOpen}
                  data-shown={shown}
                  onClick={() => setOpen(i)}
                  {...bind}
                >
                  {isOpen ? (
                    <span className="cell__slot" />
                  ) : (
                    <motion.span layoutId={`plate-${cell.id}`} transition={CLOSE} className="cell__face" style={{ borderRadius: 7 }}>
                      {cell.kind === "feature" ? (
                        <span className="cell__window">
                          <PixelCanvas w={24} h={24} make={makers[cell.scene]} reveal={shown} chunk={4} />
                        </span>
                      ) : (
                        <span className="cell__readout" aria-hidden="true">
                          {t.planned[cell.id].version}
                        </span>
                      )}
                    </motion.span>
                  )}
                </button>
              );
            })}
          </div>
        </div>
        <Legend t={t} hover={hover} stage={stage} />
      </div>
      {/* In body, so no transformed ancestor (the hero's entrance) becomes
          the containing block of the fixed layers. */}
      {portal &&
        createPortal(
          <AnimatePresence>
            {openCell && (
              <motion.div
                key="backdrop"
                className="explorer-backdrop"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0, transition: CLOSE }}
              />
            )}
            {openCell && <Explorer key="explorer" cell={openCell} t={t} onClose={close} />}
          </AnimatePresence>,
          portal,
        )}
    </MotionConfig>
  );
}

/** The strip under the plate names what the pointer rests on; during the
 * launch moment it becomes a display with the stage and a bar meter (§12). */
function Legend({ t, hover, stage }: { t: Props["t"]; hover: number | null; stage: number }) {
  const stages = t.launch.stages;
  if (stage >= 0)
    return (
      <div className="legend legend--display display" aria-live="polite">
        <span className="display__legend">
          {t.launch.legend} {stage + 1}/{stages.length}
        </span>
        <span className="legend__stage">{stages[stage]}</span>
        <span className="meter" aria-hidden="true">
          {Array.from({ length: stages.length * 3 }, (_, i) => (
            <i key={i} data-lit={i < (stage + 1) * 3} />
          ))}
        </span>
      </div>
    );

  const cell = hover === null ? null : CELLS[hover];
  let body: React.ReactNode = <span className="muted">{t.hero.idle}</span>;
  if (cell?.kind === "feature")
    body = (
      <>
        <span className="led" data-lit="true" />
        <span className="index">{pad(cell.index)}</span>
        <span>{t.features[cell.id].name}</span>
        <span className="muted">{t.hero.shipped}</span>
      </>
    );
  else if (cell?.kind === "planned")
    body = (
      <>
        <span className="led" />
        <span className="mono muted">{t.planned[cell.id].version}</span>
        <span>{t.planned[cell.id].name}</span>
        <span className="muted">{t.hero.planned}</span>
      </>
    );
  else if (cell?.kind === "launch")
    body = (
      <>
        <span className="led" data-lit="true" />
        <span>{t.hero.download}</span>
      </>
    );
  return <div className="legend">{body}</div>;
}
