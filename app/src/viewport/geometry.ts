import * as THREE from "three";
import type { Vec3 } from "../engine";
import { AXES } from "../inference";

export const UP = new THREE.Vector3(0, 0, 1);
export const r4 = (x: number) => Math.round(x * 1e4) / 1e4;
export const arr = (v: THREE.Vector3): Vec3 => [r4(v.x), r4(v.y), r4(v.z)];

/** The circle through three points: center, radius and plane normal (null when they're in a line). */
export function circleThrough(a: THREE.Vector3, m: THREE.Vector3, b: THREE.Vector3): { c: THREE.Vector3; r: number; n: THREE.Vector3 } | null {
  const u = m.clone().sub(a);
  const v = b.clone().sub(a);
  const w = u.clone().cross(v);
  const w2 = w.lengthSq();
  if (w2 < 1e-12) return null;
  const c = a.clone().add(v.clone().cross(w).multiplyScalar(u.lengthSq()).add(w.clone().cross(u).multiplyScalar(v.lengthSq())).divideScalar(2 * w2));
  return { c, r: c.distanceTo(a), n: w.normalize() };
}

/** An arc from a to b through m, as the arc command wants it: the bulge (signed, how far it bows from the chord) about normal n. */
export function bulgeThrough(a: THREE.Vector3, m: THREE.Vector3, b: THREE.Vector3, n: THREE.Vector3): number | null {
  const k = circleThrough(a, m, b);
  if (!k) return null;
  const side = n.clone().cross(b.clone().sub(a)).normalize();
  const mid = a.clone().add(b).multiplyScalar(0.5);
  const d = k.c.clone().sub(mid).dot(side);
  return m.clone().sub(mid).dot(side) > 0 ? d + k.r : d - k.r;
}

/** Two directions across a plane, lined up with the drawing axes where they lie in it. */
export function basis(n: THREE.Vector3): [THREE.Vector3, THREE.Vector3] {
  const along = (a: THREE.Vector3) => a.clone().sub(n.clone().multiplyScalar(a.dot(n)));
  const x = along(AXES[0]);
  const u = x.length() > 0.1 ? x.normalize() : Math.abs(n.dot(AXES[2])) > 0.9 ? along(AXES[1]).normalize() : AXES[2].clone().cross(n).normalize();
  return [u, n.clone().cross(u).normalize()];
}

/** Points on a circular arc from a to b, bowing `bulge` out from the chord. */
export function arcPoints(a: THREE.Vector3, b: THREE.Vector3, bulge: number, n: THREE.Vector3, k = 12): THREE.Vector3[] {
  const chord = b.clone().sub(a);
  const l = chord.length();
  if (l < 1e-9 || Math.abs(bulge) < 1e-6) return [a, b];
  const side = n.clone().cross(chord.clone().normalize()).normalize();
  const s = Math.abs(bulge);
  const r = (l * l) / 4 / (2 * s) + s / 2;
  const mid = a.clone().add(b).multiplyScalar(0.5);
  const dir = side.clone().multiplyScalar(Math.sign(bulge));
  const c = mid.clone().addScaledVector(dir, s - r);
  const u = a.clone().sub(c).normalize();
  const w = n.clone().cross(u).normalize();
  const ang = (p: THREE.Vector3) => Math.atan2(p.clone().sub(c).dot(w), p.clone().sub(c).dot(u));
  const norm = (x: number) => ((x % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI);
  const apex = norm(ang(mid.clone().addScaledVector(dir, s)));
  let end = norm(ang(b));
  if (apex > end) end -= 2 * Math.PI; // go the way that passes through the apex
  const pts: THREE.Vector3[] = [];
  for (let i = 0; i <= k; i++) {
    const t = (end * i) / k;
    pts.push(c.clone().addScaledVector(u, r * Math.cos(t)).addScaledVector(w, r * Math.sin(t)));
  }
  return pts;
}

export function offsetOutline(outer: THREE.Vector3[], n: THREE.Vector3, d: number): THREE.Vector3[] {
  const [u, v] = basis(n);
  const depth = outer[0].dot(n);
  const p2 = outer.map((p) => new THREE.Vector2(p.dot(u), p.dot(v)));
  const k = p2.length;
  const left = (e: THREE.Vector2) => new THREE.Vector2(-e.y, e.x).normalize();
  return p2.map((b, i) => {
    const a = p2[(i + k - 1) % k];
    const c = p2[(i + 1) % k];
    const n1 = left(b.clone().sub(a));
    const n2 = left(c.clone().sub(b));
    const m = n1.clone().add(n2).multiplyScalar(d / Math.max(1 + n1.dot(n2), 0.2));
    const q = b.clone().add(m);
    return u.clone().multiplyScalar(q.x).addScaledVector(v, q.y).addScaledVector(n, depth);
  });
}
