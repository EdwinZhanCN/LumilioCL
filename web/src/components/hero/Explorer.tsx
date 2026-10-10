import { useEffect, useRef } from "react";
import { motion } from "motion/react";
import { XIcon as X } from "@phosphor-icons/react";
import type { Catalog } from "../../i18n";
import { PixelCanvas } from "../../world/PixelCanvas";
import { cornerDissolve } from "../../world/scenes";
import { makers, pad, type Cell } from "./plate";

type Props = {
  cell: Exclude<Cell, { kind: "launch" }>;
  t: Pick<Catalog, "hero" | "features" | "planned">;
  onClose: () => void;
};

// Panels open over SETTLE-ish travel with ease_out_quint; the way back is
// shorter and never bounces (design language §2).
export const OPEN = { duration: 0.42, ease: [0.22, 1, 0.36, 1] } as const;
export const CLOSE = { duration: 0.24, ease: [0.22, 1, 0.36, 1] } as const;

/** A cell grown into a window: the tile's face is the window's frame
 * (`layoutId`), so it opens out of the plate and closes back into it. */
export function Explorer({ cell, t, onClose }: Props) {
  const dialog = useRef<HTMLDivElement>(null);
  const close = useRef<HTMLButtonElement>(null);
  const titleId = `explorer-${cell.id}`;

  useEffect(() => {
    close.current?.focus({ preventScroll: true });
    const root = document.documentElement;
    const overflow = root.style.overflow;
    root.style.overflow = "hidden";
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      } else if (event.key === "Tab" && dialog.current) {
        // Keep focus inside the window.
        const focusable = dialog.current.querySelectorAll<HTMLElement>("button, a[href]");
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        if (event.shiftKey && document.activeElement === first) {
          event.preventDefault();
          last.focus();
        } else if (!event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          first.focus();
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      root.style.overflow = overflow;
      window.removeEventListener("keydown", onKey);
    };
  }, [onClose]);

  const closeKey = (
    <button ref={close} type="button" className="key key--black key--icon" onClick={onClose} aria-label={t.hero.close} title={t.hero.close}>
      <X weight="bold" />
    </button>
  );

  return (
    <div className="explorer-layer" onClick={onClose}>
      <motion.div
        ref={dialog}
        layoutId={`plate-${cell.id}`}
        transition={OPEN}
        className={`explorer explorer--${cell.kind}`}
        style={{ borderRadius: 8 }}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onClick={(event) => event.stopPropagation()}
      >
        <motion.div
          layout
          className="explorer__content"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1, transition: { delay: 0.14, duration: 0.22 } }}
          exit={{ opacity: 0, transition: { duration: 0.06 } }}
        >
          {cell.kind === "feature" ? <FeatureBody cell={cell} t={t} titleId={titleId} closeKey={closeKey} /> : <PlannedBody cell={cell} t={t} titleId={titleId} closeKey={closeKey} />}
        </motion.div>
      </motion.div>
    </div>
  );
}

function FeatureBody({
  cell,
  t,
  titleId,
  closeKey,
}: {
  cell: Extract<Cell, { kind: "feature" }>;
  t: Props["t"];
  titleId: string;
  closeKey: React.ReactNode;
}) {
  const feature = t.features[cell.id];
  return (
    <>
      <PixelCanvas w={72} h={24} make={makers[cell.scene]} post={cornerDissolve} className="explorer__cover" />
      <span className="screws" aria-hidden="true" />
      <header className="explorer__head">
        <span className="index">{pad(cell.index)}</span>
        <h2 id={titleId} className="explorer__title">
          {feature.name}
        </h2>
        <span className="tag tag--ink">{t.hero.shipped}</span>
        <span className="explorer__close">{closeKey}</span>
      </header>
      <div className="explorer__body">
        <figure className="window explorer__shot">
          {cell.shot ? (
            <img src={cell.shot} alt={feature.shot} loading="lazy" decoding="async" />
          ) : (
            // TODO(asset): replace with the screenshot or GIF named in `shot`.
            <div className="placeholder" role="img" aria-label={feature.shot}>
              <span className="silk">{feature.shot}</span>
            </div>
          )}
        </figure>
        <div className="explorer__text">
          <p className="lede">{feature.lede}</p>
          <ul className="facts">
            {feature.facts.map((fact) => (
              <li key={fact}>{fact}</li>
            ))}
          </ul>
        </div>
      </div>
    </>
  );
}

function PlannedBody({
  cell,
  t,
  titleId,
  closeKey,
}: {
  cell: Extract<Cell, { kind: "planned" }>;
  t: Props["t"];
  titleId: string;
  closeKey: React.ReactNode;
}) {
  const plan = t.planned[cell.id];
  return (
    <>
      <span className="screws" aria-hidden="true" />
      <header className="explorer__head">
        <h2 id={titleId} className="explorer__title">
          {plan.name}
        </h2>
        <span className="tag">{t.hero.planned}</span>
        <span className="explorer__close">{closeKey}</span>
      </header>
      <div className="display explorer__readout">
        <span className="display__legend">VERSION</span>
        <span className="digits explorer__digits" data-ghost={plan.version.replace(/\d/g, "8")}>
          <span>{plan.version}</span>
        </span>
      </div>
      <p className="lede">{plan.lede}</p>
    </>
  );
}
