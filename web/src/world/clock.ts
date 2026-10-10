// One clock for every world animation on the page, at the game's own rate
// (WORLD_TICK, 20 ticks/s; design language §2). It runs only while some
// visible canvas listens, and not at all while the tab is hidden.

export const WORLD_TICK_MS = 50;

type Listener = (tick: number) => void;

const listeners = new Set<Listener>();
let tick = 0;
let frame = 0;
let last = 0;

function loop(now: number) {
  if (!last) last = now;
  // At most a few ticks per frame, so a throttled tab does not fast-forward.
  let steps = 0;
  while (now - last >= WORLD_TICK_MS && steps < 3) {
    last += WORLD_TICK_MS;
    tick += 1;
    steps += 1;
  }
  if (now - last >= WORLD_TICK_MS) last = now;
  if (steps) for (const listener of listeners) listener(tick);
  frame = listeners.size ? requestAnimationFrame(loop) : 0;
}

export function listen(listener: Listener): () => void {
  listeners.add(listener);
  if (!frame) {
    last = 0;
    frame = requestAnimationFrame(loop);
  }
  return () => {
    listeners.delete(listener);
    if (!listeners.size && frame) {
      cancelAnimationFrame(frame);
      frame = 0;
    }
  };
}

export const currentTick = () => tick;
