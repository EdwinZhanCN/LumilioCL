// The world register's raster (design language §1): procedural pixel art,
// painted texel by texel into an ImageData and scaled up with
// `image-rendering: pixelated`. A small web counterpart of
// crates/lumilio-ui/src/hero/raster.rs.

export type Rgb = readonly [number, number, number];

export const hex = (n: number): Rgb => [(n >> 16) & 255, (n >> 8) & 255, n & 255];

export const lerp = (a: Rgb, b: Rgb, t: number): Rgb => [
  a[0] + (b[0] - a[0]) * t,
  a[1] + (b[1] - a[1]) * t,
  a[2] + (b[2] - a[2]) * t,
];

export const scale = (c: Rgb, k: number): Rgb => [c[0] * k, c[1] * k, c[2] * k];

export const add = (a: Rgb, b: Rgb): Rgb => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];

export class Grid {
  readonly data: Uint8ClampedArray<ArrayBuffer>;

  constructor(
    readonly w: number,
    readonly h: number,
  ) {
    this.data = new Uint8ClampedArray(w * h * 4);
  }

  clear(): void {
    this.data.fill(0);
  }

  put(x: number, y: number, c: Rgb, alpha = 255): void {
    x = Math.floor(x);
    y = Math.floor(y);
    if (x < 0 || y < 0 || x >= this.w || y >= this.h) return;
    const i = (y * this.w + x) * 4;
    this.data[i] = c[0];
    this.data[i + 1] = c[1];
    this.data[i + 2] = c[2];
    this.data[i + 3] = alpha;
  }

  get(x: number, y: number): Rgb {
    const i = (Math.floor(y) * this.w + Math.floor(x)) * 4;
    return [this.data[i], this.data[i + 1], this.data[i + 2]];
  }

  /** Mixes `c` over what is already there; an empty texel takes `c` whole. */
  blend(x: number, y: number, c: Rgb, t: number): void {
    x = Math.floor(x);
    y = Math.floor(y);
    if (x < 0 || y < 0 || x >= this.w || y >= this.h) return;
    const i = (y * this.w + x) * 4;
    if (this.data[i + 3] === 0) {
      this.put(x, y, c);
      return;
    }
    this.put(x, y, lerp(this.get(x, y), c, Math.min(1, Math.max(0, t))));
  }

  rect(x: number, y: number, w: number, h: number, c: Rgb): void {
    for (let dy = 0; dy < h; dy++) for (let dx = 0; dx < w; dx++) this.put(x + dx, y + dy, c);
  }

  /** Clears texels outside the first `shown` of `order`'s chunks: the
   * chunk-by-chunk reveal of "the world is becoming ready" (§3). */
  mask(chunk: number, order: readonly number[], shown: number): void {
    if (shown >= order.length) return;
    const cols = Math.ceil(this.w / chunk);
    const hidden = new Set(order.slice(Math.max(0, shown)));
    for (let y = 0; y < this.h; y++) {
      for (let x = 0; x < this.w; x++) {
        const id = Math.floor(y / chunk) * cols + Math.floor(x / chunk);
        if (hidden.has(id)) this.data[(y * this.w + x) * 4 + 3] = 0;
      }
    }
  }
}

/** A stable shuffled order of a grid's chunks, so a reveal looks loaded
 * rather than wiped. */
export function chunkOrder(w: number, h: number, chunk: number, seed: number): number[] {
  const cols = Math.ceil(w / chunk);
  const rows = Math.ceil(h / chunk);
  const ids = Array.from({ length: cols * rows }, (_, i) => i);
  return ids
    .map((id) => ({ id, key: hash(id, rows, seed) + (Math.floor(id / cols) / rows) * 0.6 }))
    .sort((a, b) => b.key - a.key)
    .map(({ id }) => id);
}

/** Integer hash to [0, 1). */
export function hash(x: number, y: number, seed: number): number {
  let h = Math.imul(x | 0, 374761393) ^ Math.imul(y | 0, 668265263) ^ Math.imul(seed | 0, 2246822519);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  h ^= h >>> 16;
  return (h >>> 0) / 4294967296;
}

const smooth = (t: number) => t * t * (3 - 2 * t);

/** Value noise in [0, 1). */
export function noise(x: number, y: number, seed: number): number {
  const x0 = Math.floor(x);
  const y0 = Math.floor(y);
  const fx = smooth(x - x0);
  const fy = smooth(y - y0);
  const a = hash(x0, y0, seed);
  const b = hash(x0 + 1, y0, seed);
  const c = hash(x0, y0 + 1, seed);
  const d = hash(x0 + 1, y0 + 1, seed);
  return a + (b - a) * fx + (c - a) * fy + (a - b - c + d) * fx * fy;
}

export function fbm(x: number, y: number, seed: number, octaves = 4): number {
  let sum = 0;
  let amp = 0.5;
  let norm = 0;
  for (let o = 0; o < octaves; o++) {
    sum += noise(x, y, seed + o * 101) * amp;
    norm += amp;
    x *= 2;
    y *= 2;
    amp *= 0.5;
  }
  return sum / norm;
}

/** Steps a continuous value to `levels`, the stepped corners of pixel light. */
export const quantize = (v: number, levels: number) => Math.round(v * levels) / levels;

/** 4×4 ordered dither threshold in [0, 1). */
const BAYER = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
export const bayer = (x: number, y: number) => BAYER[(y & 3) * 4 + (x & 3)] / 16;
