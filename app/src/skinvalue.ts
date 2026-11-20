// A shared theme (.sketchskin) is data from someone else, and its values end
// up in CSS custom properties that the stylesheet uses as colors, sizes and
// fonts. So each value must be exactly one of those, written in a plain form:
// a deny-list of bad words can't keep up with CSS (image-set(), escapes...).
// Pure and dependency-free so checks/run.mjs can test it.

const NUM = String.raw`-?(?:\d+\.?\d*|\.\d+)`;
const HEX = /^#(?:[0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i;
// rgb(...) rgba(...) hsl(...) hsla(...): numbers, %, deg, commas, slashes, spaces
const FUNC = new RegExp(String.raw`^(?:rgb|rgba|hsl|hsla)\(\s*${NUM}(?:%|deg)?(?:\s*[, /]\s*${NUM}(?:%|deg)?){2,3}\s*\)$`, "i");
// plain color words: "white", "transparent", "currentcolor"
const WORD = /^[a-z]{3,24}$/i;
// 4px, 0.5rem, 12px 8px, 50%
const LENGTH = new RegExp(String.raw`^${NUM}(?:px|rem|em|%|pt)?(?:\s+${NUM}(?:px|rem|em|%|pt)?){0,3}$`, "i");
// one font name: "Bricolage Grotesque Variable", 'IBM Plex Mono', Georgia, system-ui
const QUOTED = /^(?:"[A-Za-z0-9 ._-]{1,60}"|'[A-Za-z0-9 ._-]{1,60}')$/;
const BARE = /^-?[A-Za-z][A-Za-z0-9 _-]{0,60}$/; // -apple-system, Georgia, system-ui

export function colorOk(v: string): boolean {
  return HEX.test(v) || FUNC.test(v) || WORD.test(v);
}

export function lengthOk(v: string): boolean {
  return LENGTH.test(v);
}

export function fontOk(v: string): boolean {
  const parts = v.split(",").map((s) => s.trim());
  return parts.length >= 1 && parts.length <= 12 && parts.every((p) => QUOTED.test(p) || BARE.test(p));
}

/** Is `value` something this token may hold? */
export function skinValueOk(token: string, value: unknown): value is string {
  if (typeof value !== "string" || value.length === 0 || value.length >= 200) return false;
  if (token === "radius") return lengthOk(value);
  if (token === "font") return fontOk(value);
  return colorOk(value);
}
