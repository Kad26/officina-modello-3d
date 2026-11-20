import { type Backdrop } from "./types";

/** A pleasant color for a tag that hasn't been given one (stable per name). */
export function autoTagColor(tag: string): [number, number, number] {
  let h = 0;
  for (const ch of tag) h = (h * 31 + ch.charCodeAt(0)) >>> 0;
  const hue = (h % 360) / 360;
  // soft, distinct: HSL with s .5, l .68
  const f = (n: number) => {
    const k = (n + hue * 12) % 12;
    return 0.68 - 0.5 * Math.min(0.68, 1 - 0.68) * Math.max(-1, Math.min(k - 3, 9 - k, 1));
  };
  return [f(0), f(8), f(4)];
}

export const ACCENT = 0x6b5cff;
const EDGE_COLOR = 0x23252d;

/** Clear color, line color, and whether the ground grid shows, per background. */
export const BACKGROUNDS: Record<Backdrop, { clear: number; line: number; face: number; grid: boolean }> = {
  default: { clear: 0xf3f4f8, line: EDGE_COLOR, face: 0xffffff, grid: true },
  paper: { clear: 0xf6f1e4, line: 0x2b2a27, face: 0xfbf8f0, grid: false },
  blueprint: { clear: 0x1d4f8c, line: 0xeaf2ff, face: 0x1d4f8c, grid: false },
  night: { clear: 0x16181f, line: 0xd8dbe6, face: 0x2a2d38, grid: false },
};
