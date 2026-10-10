import { useEffect, useRef, type CSSProperties } from "react";
import { useReducedMotion } from "motion/react";
import { listen } from "./clock";
import { Grid, chunkOrder } from "./grid";
import type { Scene } from "./scenes";

type Props = {
  w: number;
  h: number;
  make: (w: number, h: number) => Scene;
  /** Whether the world is shown; it loads and unloads chunk by chunk. */
  reveal?: boolean;
  /** Chunk size in texels for the reveal; without it the scene just appears. */
  chunk?: number;
  /** Runs after the scene each frame, e.g. the dissolve into the page. */
  post?: (g: Grid) => void;
  /** The tick drawn as the still frame under reduced motion. */
  still?: number;
  label?: string;
  className?: string;
  style?: CSSProperties;
};

const CHUNKS_IN = 3;
const CHUNKS_OUT = 6;

export function PixelCanvas({ w, h, make, reveal = true, chunk, post, still = 40, label, className, style }: Props) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const target = useRef(reveal);
  const kick = useRef<() => void>(() => {});
  const reduce = useReducedMotion() ?? false;
  const postRef = useRef(post);
  postRef.current = post;

  useEffect(() => {
    const element = canvas.current;
    const context = element?.getContext("2d");
    if (!element || !context) return;
    const grid = new Grid(w, h);
    const image = new ImageData(grid.data, w, h);
    const scene = make(w, h);
    const order = chunk ? chunkOrder(w, h, chunk, w * 31 + h) : [];
    let shown = target.current || !chunk ? order.length : 0;
    let visible = false;
    let stop: (() => void) | null = null;

    const paint = (tick: number) => {
      grid.clear();
      if (shown > 0 || !chunk) {
        scene(grid, tick);
        if (chunk) grid.mask(chunk, order, shown);
        postRef.current?.(grid);
      }
      context.putImageData(image, 0, 0);
    };

    const settle = () => {
      const goal = target.current ? order.length : 0;
      if (shown < goal) shown = Math.min(goal, shown + CHUNKS_IN);
      else if (shown > goal) shown = Math.max(goal, shown - CHUNKS_OUT);
    };

    const sync = () => {
      const goal = target.current ? order.length : 0;
      const busy = visible && !reduce && (target.current || shown !== goal);
      if (busy && !stop) {
        stop = listen((tick) => {
          settle();
          paint(tick);
          sync();
        });
      } else if (!busy && stop) {
        stop();
        stop = null;
      }
    };

    kick.current = () => {
      if (reduce) {
        shown = target.current || !chunk ? order.length : 0;
        paint(still);
      }
      sync();
    };

    const observer = new IntersectionObserver(([entry]) => {
      visible = entry.isIntersecting;
      sync();
    });
    observer.observe(element);
    paint(still);

    return () => {
      observer.disconnect();
      stop?.();
    };
  }, [w, h, make, chunk, reduce, still]);

  useEffect(() => {
    target.current = reveal;
    kick.current();
  }, [reveal]);

  return (
    <canvas
      ref={canvas}
      width={w}
      height={h}
      className={["pixelated", className].filter(Boolean).join(" ")}
      style={style}
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
    />
  );
}
