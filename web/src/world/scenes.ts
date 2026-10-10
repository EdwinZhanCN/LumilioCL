// Pixel vignettes for the site, in the palettes of the launcher's hero scenes
// (crates/lumilio-ui/src/hero/scenes/*.rs). Each scene is drawn at any texel
// size from proportions, and is a pure function of the tick, so a still frame
// (reduced motion) is just one tick.

import { Grid, type Rgb, add, bayer, fbm, hash, hex, lerp, noise, quantize, scale } from "./grid";

export type Scene = (g: Grid, tick: number) => void;
export type SceneId = "dawn" | "portal" | "map" | "redstone" | "caves";

const stops = (colors: readonly Rgb[], t: number): Rgb => {
  const x = Math.min(0.9999, Math.max(0, t)) * (colors.length - 1);
  const i = Math.floor(x);
  return lerp(colors[i], colors[i + 1], x - i);
};

const twinkle = (x: number, y: number, tick: number, seed: number) => {
  const phase = Math.floor(hash(x, y, seed + 1) * 60);
  return (tick + phase) % 60 < 6 ? 1 : 0.45;
};

// ---- Dawn: the library's first light over a homestead ----

function dawn(w: number, h: number): Scene {
  const sky = [hex(0x3b4a7a), hex(0x7a5f99), hex(0xd9789a), hex(0xf7a86b), hex(0xffd08a)];
  const horizon = Math.round(h * 0.62);
  const ground = (x: number) => Math.round(h * 0.7 - fbm(x * 0.05, 3, 17, 3) * h * 0.08);
  const ridge = (x: number) => Math.round(h * 0.56 - fbm(x * 0.04, 9, 7, 3) * h * 0.2);
  const sun = { x: w * 0.7, y: horizon * 0.72, r: Math.max(2, Math.min(w, h) * 0.09) };
  const house = { x: Math.round(w * 0.16), w: Math.max(5, Math.round(w * 0.2)) };
  house.w += house.w % 2 === 0 ? 1 : 0;

  return (g, tick) => {
    for (let y = 0; y < h; y++) {
      const band = sky.length > 1 ? quantize(y / horizon, 7) : 0;
      for (let x = 0; x < w; x++) {
        let c = stops(sky, band);
        const d = Math.hypot(x - sun.x, y - sun.y) / sun.r;
        if (d < 1) c = d < 0.7 ? hex(0xfff1c4) : hex(0xffd27a);
        else if (d < 2.6) c = lerp(c, hex(0xffe0a0), quantize((2.6 - d) / 1.6, 3) * 0.45);
        g.put(x, y, c);
      }
    }
    // Clouds drift one texel every 10 ticks.
    for (let i = 0; i < 3; i++) {
      const cw = Math.max(4, Math.round(w * (0.18 + 0.08 * i)));
      const cx = ((Math.floor(tick / 10) + i * Math.round(w * 0.45)) % (w + cw * 2)) - cw;
      const cy = Math.round(h * (0.14 + 0.12 * i));
      for (let dx = 0; dx < cw; dx++) {
        const top = Math.round(noise(dx * 0.35, i * 9, 5) * 2.5);
        const depth = Math.max(1, Math.round(h * 0.04));
        for (let dy = -top; dy < depth; dy++)
          g.put(cx + dx, cy + dy, dy === depth - 1 ? hex(0xe8a98a) : hex(0xffe3c4));
      }
    }
    for (let x = 0; x < w; x++) {
      const r = ridge(x);
      for (let y = r; y < h; y++) g.put(x, y, y === r ? hex(0x9b88b0) : hex(0x806f9a));
      const top = ground(x);
      for (let y = top; y < h; y++) {
        const depth = y - top;
        const c =
          depth === 0
            ? hex(0x7fbf55)
            : depth === 1
              ? hex(0x5d9b47)
              : hash(x, y, 3) > 0.8
                ? hex(0x6b4729)
                : hex(0x7a5232);
        g.put(x, y, c);
      }
    }
    // A homestead with its window lit.
    const base = ground(house.x + Math.floor(house.w / 2));
    const wallH = Math.max(3, Math.round(h * 0.13));
    for (let dy = 0; dy < wallH; dy++)
      for (let dx = 0; dx < house.w; dx++)
        g.put(house.x + dx, base - 1 - dy, dy % 3 === 2 ? hex(0x8f6c3e) : hex(0xb08a55));
    const roofH = Math.ceil(house.w / 2);
    for (let r = 0; r < roofH; r++)
      for (let dx = r; dx < house.w - r; dx++)
        g.put(house.x + dx, base - 1 - wallH - r, r === 0 ? hex(0x4f3018) : hex(0x6b4423));
    const win = Math.max(1, Math.round(house.w * 0.22));
    const flicker = hash(Math.floor(tick / 7), 1, 9) > 0.8 ? hex(0xffe9a8) : hex(0xffd27a);
    g.rect(house.x + Math.round(house.w * 0.62) - Math.floor(win / 2), base - 1 - Math.round(wallH * 0.7), win, win, flicker);
    g.rect(house.x + Math.round(house.w * 0.2), base - Math.max(2, Math.round(wallH * 0.6)), Math.max(1, win), Math.max(2, Math.round(wallH * 0.6)), hex(0x5a3a22));
  };
}

// ---- Portal: Discover, a way to somewhere else ----

function portal(w: number, h: number): Scene {
  const groundY = Math.round(h * 0.8);
  const t = Math.max(1, Math.round(w * 0.06));
  const iw = Math.max(3, Math.round(w * 0.3));
  const ih = Math.max(4, Math.round(h * 0.42));
  const px = Math.round((w - iw) / 2) - t;
  const py = groundY - ih - t * 2;
  const swirl = [hex(0x3a0f7a), hex(0x5a1fb0), hex(0x7b33dc), hex(0xa25cf5), hex(0xc995ff)];

  return (g, tick) => {
    for (let y = 0; y < h; y++)
      for (let x = 0; x < w; x++) {
        let c = lerp(hex(0x0b0d24), hex(0x1a1d42), quantize(y / groundY, 4));
        if (y < groundY && hash(x, y, 77) > 0.975) c = lerp(c, hex(0xeef0ff), twinkle(x, y, tick, 77));
        if (y >= groundY) c = y === groundY ? hex(0x1f3a24) : hash(x, y, 8) > 0.75 ? hex(0x22170f) : hex(0x2a1d14);
        g.put(x, y, c);
      }
    for (let y = py; y < groundY; y++)
      for (let x = px; x < px + iw + t * 2; x++) {
        const inside = x >= px + t && x < px + t + iw && y >= py + t && y < groundY - t;
        if (inside) {
          const v = fbm(x * 0.28 + Math.sin((y + tick * 0.4) * 0.3) * 0.6, y * 0.22 - tick * 0.12, 61, 3);
          g.put(x, y, stops(swirl, quantize(v, 4)));
        } else g.put(x, y, hash(x, y, 13) > 0.8 ? hex(0x2b1b45) : hex(0x140c22));
      }
    // Light spilling on the grass in front of it, stepped.
    for (let x = 0; x < w; x++) {
      const d = Math.abs(x - w / 2) / (iw * 0.9);
      if (d < 1) for (let y = groundY; y < groundY + 2; y++) g.blend(x, y, hex(0x7b33dc), quantize(1 - d, 3) * 0.5);
    }
    for (let i = 0; i < 6; i++) {
      const rise = (tick * 0.5 + i * 17) % (ih + 6);
      const x = px + t + Math.floor(hash(i, Math.floor((tick * 0.5 + i * 17) / (ih + 6)), 5) * iw);
      g.put(x, groundY - t - rise, hex(0xe2c4ff));
    }
  };
}

// ---- Map: the world explorer, seen from above ----

function map(w: number, h: number): Scene {
  const biome = (wx: number, wy: number) => {
    const e = fbm(wx * 0.045, wy * 0.045, 11, 4);
    const m = fbm(wx * 0.03 + 50, wy * 0.03, 23, 3);
    if (e < 0.38) return hex(0x2b4c8c);
    if (e < 0.45) return hex(0x3a64b0);
    if (e < 0.47) return hex(0xd9cc8a);
    if (e < 0.6) return m > 0.56 ? hex(0x4f8a35) : m < 0.32 ? hex(0xd8c48a) : hex(0x79b04a);
    if (e < 0.68) return hex(0x6f8a4a);
    if (e < 0.75) return hex(0x8f8f88);
    return hex(0xeef0f2);
  };
  const height = (wx: number, wy: number) => fbm(wx * 0.045, wy * 0.045, 11, 4);

  return (g, tick) => {
    const ox = Math.floor(tick / 4);
    const oy = Math.floor(tick / 11);
    for (let y = 0; y < h; y++)
      for (let x = 0; x < w; x++) {
        const wx = x + ox;
        const wy = y + oy;
        let c = biome(wx, wy);
        const e = height(wx, wy);
        if (e >= 0.45) {
          const slope = e - height(wx - 1, wy - 1);
          c = scale(c, 1 + quantize(Math.max(-1, Math.min(1, slope * 40)), 2) * 0.12);
        }
        if (wx % 16 === 0 || wy % 16 === 0) c = scale(c, 0.88);
        g.put(x, y, c);
      }
    // Waypoints sit on the world and pass by as it pans.
    const cell = 40;
    for (let cy = Math.floor(oy / cell) - 1; cy <= Math.floor((oy + h) / cell) + 1; cy++)
      for (let cx = Math.floor(ox / cell) - 1; cx <= Math.floor((ox + w) / cell) + 1; cx++) {
        if (hash(cx, cy, 91) < 0.45) continue;
        const sx = cx * cell + Math.floor(hash(cx, cy, 92) * cell) - ox;
        const sy = cy * cell + Math.floor(hash(cx, cy, 93) * cell) - oy;
        const color = hash(cx, cy, 94) > 0.5 ? hex(0xff5a1a) : hex(0x5ee3e0);
        g.put(sx, sy - 1, color);
        g.put(sx - 1, sy, color);
        g.put(sx + 1, sy, color);
        g.put(sx, sy + 1, color);
        g.put(sx, sy, hex(0xffffff));
      }
    // The player, always at the centre.
    const mx = Math.floor(w / 2);
    const my = Math.floor(h / 2);
    g.put(mx, my - 1, hex(0xffffff));
    g.put(mx - 1, my, hex(0xffffff));
    g.put(mx, my, hex(0xffffff));
    g.put(mx + 1, my, hex(0xffffff));
    g.put(mx, my + 1, hex(0x1d1d1d));
  };
}

// ---- Redstone: a signal walking to a lamp (the schematic tile) ----

function redstone(w: number, h: number): Scene {
  const floor = Math.round(h * 0.78);
  const start = Math.max(1, Math.round(w * 0.12));
  const lampS = Math.max(4, Math.round(Math.min(w, h) * 0.24));
  const lampX = w - lampS - Math.max(1, Math.round(w * 0.08));
  const end = lampX - 1;
  const len = Math.max(1, end - start);
  const brickW = Math.max(4, Math.round(w / 6));
  const brickH = Math.max(2, Math.round(brickW / 2));

  return (g, tick) => {
    const cycle = len + 34;
    const p = tick % cycle;
    const lit = p >= len && p < len + 22;
    for (let y = 0; y < h; y++)
      for (let x = 0; x < w; x++) {
        let c: Rgb;
        if (y < floor) {
          const row = Math.floor(y / brickH);
          const bx = x + (row % 2 ? Math.floor(brickW / 2) : 0);
          const mortar = y % brickH === 0 || bx % brickW === 0;
          c = mortar ? hex(0x4f4f52) : hash(Math.floor(bx / brickW), row, 4) > 0.5 ? hex(0x6e6e72) : hex(0x78787c);
          if (lit) {
            const d = Math.hypot(x - (lampX + lampS / 2), y - (floor - lampS / 2)) / (w * 0.6);
            if (d < 1) c = lerp(c, hex(0xffcf7a), quantize(1 - d, 3) * 0.35);
          }
        } else c = y === floor ? hex(0x9a9a9e) : hex(0x5e5e62);
        g.put(x, y, c);
      }
    // Lever.
    g.rect(start - 1, floor - 2, 2, 2, hex(0x5e5e62));
    g.put(start - (p < len + 22 ? 0 : 1), floor - 3, hex(0x8f6c3e));
    // Dust: power falls off with distance, 15 levels like the game's.
    for (let x = start; x <= end; x++) {
      const travelled = x - start;
      const on = !lit && p < len ? travelled <= p : lit;
      const power = on ? 15 - Math.floor((travelled / len) * 12) : 0;
      const c = lerp(hex(0x4a0a0a), hex(0xff2a1a), power / 15);
      g.put(x, floor - 1, (x + 1) % 2 ? c : scale(c, 0.8));
    }
    // Lamp.
    for (let dy = 0; dy < lampS; dy++)
      for (let dx = 0; dx < lampS; dx++) {
        const edge = dx === 0 || dy === 0 || dx === lampS - 1 || dy === lampS - 1;
        const lattice = (dx + dy) % 3 === 0;
        const c = edge
          ? hex(0x3a2616)
          : lit
            ? lattice
              ? hex(0xfff1b0)
              : hex(0xffcf6a)
            : lattice
              ? hex(0x5a3f22)
              : hex(0x7a5a30);
        g.put(lampX + dx, floor - lampS + dy, c);
      }
  };
}

// ---- Caves: digging into what went wrong (diagnostics) ----

function caves(w: number, h: number): Scene {
  const ores: Rgb[] = [hex(0x5ee3e0), hex(0xf5d24a), hex(0xd8b08c), hex(0x141414)];
  const lavaY = Math.round(h * 0.84);
  const torch = { x: Math.round(w * 0.28), y: Math.round(h * 0.38) };

  return (g, tick) => {
    const flick = hash(Math.floor(tick / 2), 0, 3) > 0.5 ? 1 : 0.8;
    for (let y = 0; y < h; y++)
      for (let x = 0; x < w; x++) {
        const v = fbm(x * 0.2, y * 0.2, 31, 3);
        let c = stops([hex(0x26262b), hex(0x34343a), hex(0x404047)], quantize(v, 2));
        const hollow = fbm(x * 0.09, y * 0.13, 41, 3) > 0.55;
        if (hollow) c = hex(0x0c0c10);
        else {
          const ore = hash(x, y, 51);
          if (ore > 0.965) {
            c = ores[Math.floor(hash(x, y, 52) * ores.length)];
            if ((tick + Math.floor(hash(x, y, 53) * 90)) % 90 < 2) c = hex(0xffffff);
          }
        }
        if (y >= lavaY) {
          const l = noise(x * 0.3 + tick * 0.05, y * 0.4 - tick * 0.02, 71);
          c = stops([hex(0xc24a18), hex(0xff6a1a), hex(0xffb43a)], quantize(l, 2));
        } else {
          const glow = 1 - (lavaY - y) / (h * 0.35);
          if (glow > 0) c = lerp(c, hex(0xff6a1a), quantize(glow, 3) * 0.3);
          const d = Math.hypot(x - torch.x, y - torch.y) / (w * 0.3);
          if (d < 1 && !hollow) c = add(c, scale(hex(0x6a4a20), quantize(1 - d, 3) * flick));
        }
        g.put(x, y, c);
      }
    g.rect(torch.x, torch.y, 1, Math.max(2, Math.round(h * 0.08)), hex(0x6b4a2f));
    g.put(torch.x, torch.y - 1, flick === 1 ? hex(0xffd27a) : hex(0xff8a2a));
  };
}

const factories: Record<SceneId, (w: number, h: number) => Scene> = { dawn, portal, map, redstone, caves };

export const makeScene = (id: SceneId, w: number, h: number): Scene => factories[id](w, h);

/** A cover in the top-right corner that dissolves leftward and downward into
 * whatever surface sits under the canvas, like the Instance page's cover
 * (§7): dissolved texels are cleared, not painted. */
export function cornerDissolve(g: Grid, from = 0.45): void {
  for (let y = 0; y < g.h; y++)
    for (let x = 0; x < g.w; x++) {
      const left = (g.w - 1 - x) / g.w;
      const d = Math.max(left, (y / g.h) * 0.9 + left * 0.35);
      const t = (d - from) / (1 - from);
      if (t > 0 && bayer(x, y) < t * 1.4) g.data[(y * g.w + x) * 4 + 3] = 0;
    }
}

