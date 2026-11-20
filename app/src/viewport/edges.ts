import * as THREE from "three";

/** Little deterministic noise for sketchy lines. */
function jitter(i: number): number {
  const x = Math.sin(i * 12.9898 + 78.233) * 43758.5453;
  return x - Math.floor(x) - 0.5;
}

/** Edge segments, lengthened past their ends and/or doubled and wobbled. */
export function styledEdges(seg: number[], size: number, extend: boolean, sketchy: boolean): number[] {
  const out: number[] = [];
  const a = new THREE.Vector3(), b = new THREE.Vector3(), d = new THREE.Vector3(), side = new THREE.Vector3();
  const ext = extend ? size * 0.012 : 0;
  for (let i = 0; i < seg.length; i += 6) {
    a.fromArray(seg, i);
    b.fromArray(seg, i + 3);
    d.subVectors(b, a);
    const len = d.length();
    if (len < 1e-9) continue;
    d.divideScalar(len);
    const copies = sketchy ? 2 : 1;
    // run-ons and wobble scale with the edge itself, so small things stay tidy
    const cap = Math.min(ext, len * 0.18);
    const wob = Math.min(size * 0.003, len * 0.02);
    for (let c = 0; c < copies; c++) {
      const k = i / 6 + c * 7919;
      const e0 = cap + (sketchy ? Math.abs(jitter(k)) * Math.min(size * 0.01, len * 0.12) : 0);
      const e1 = cap + (sketchy ? Math.abs(jitter(k + 1)) * Math.min(size * 0.01, len * 0.12) : 0);
      const p = a.clone().addScaledVector(d, -e0);
      const q = b.clone().addScaledVector(d, e1);
      if (sketchy) {
        // a small sideways wobble, different at each end
        side.set(-d.y, d.x, d.z * 0.3).normalize();
        if (side.lengthSq() < 1e-6) side.set(1, 0, 0);
        p.addScaledVector(side, jitter(k + 2) * wob);
        q.addScaledVector(side, jitter(k + 3) * wob);
      }
      out.push(p.x, p.y, p.z, q.x, q.y, q.z);
    }
  }
  return out;
}

/** Normals averaged at every shared corner (for the profile outline). */
export function averagedNormals(pos: number[]): Float32Array {
  const acc = new Map<string, THREE.Vector3>();
  const key = (i: number) => `${Math.round(pos[i] * 1e4)},${Math.round(pos[i + 1] * 1e4)},${Math.round(pos[i + 2] * 1e4)}`;
  const a = new THREE.Vector3(), b = new THREE.Vector3(), c = new THREE.Vector3();
  for (let t = 0; t < pos.length; t += 9) {
    a.fromArray(pos, t);
    b.fromArray(pos, t + 3);
    c.fromArray(pos, t + 6);
    const n = b.clone().sub(a).cross(c.clone().sub(a)).normalize();
    for (let k = 0; k < 3; k++) {
      const id = key(t + k * 3);
      const v = acc.get(id);
      if (v) v.add(n);
      else acc.set(id, n.clone());
    }
  }
  const out = new Float32Array(pos.length);
  for (let v = 0; v < pos.length; v += 3) {
    const n = acc.get(key(v))!.clone().normalize();
    out[v] = n.x;
    out[v + 1] = n.y;
    out[v + 2] = n.z;
  }
  return out;
}

/**
 * Per-corner normals for a triangle soup: corners where neighboring
 * triangles meet at under 35 degrees share an averaged normal (smooth
 * ground, cylinders), sharper corners keep the triangle's own (crisp boxes).
 */
export function smoothNormals(pos: number[]): Float32Array {
  const n = pos.length / 9;
  const face = new Float32Array(n * 3);
  const a = new THREE.Vector3(), b = new THREE.Vector3(), c = new THREE.Vector3();
  for (let t = 0; t < n; t++) {
    a.fromArray(pos, t * 9);
    b.fromArray(pos, t * 9 + 3);
    c.fromArray(pos, t * 9 + 6);
    // not normalized: bigger triangles count more in the average
    const f = b.sub(a).cross(c.sub(a));
    face[t * 3] = f.x;
    face[t * 3 + 1] = f.y;
    face[t * 3 + 2] = f.z;
  }
  const key = (i: number) => `${Math.round(pos[i] * 1e4)},${Math.round(pos[i + 1] * 1e4)},${Math.round(pos[i + 2] * 1e4)}`;
  const at = new Map<string, number[]>();
  for (let v = 0; v < n * 3; v++) {
    const k = key(v * 3);
    const l = at.get(k);
    if (l) l.push(v);
    else at.set(k, [v]);
  }
  const out = new Float32Array(n * 9);
  const cos = Math.cos((35 * Math.PI) / 180);
  const own = new THREE.Vector3(), other = new THREE.Vector3(), sum = new THREE.Vector3();
  for (const corners of at.values()) {
    for (const v of corners) {
      const t = Math.floor(v / 3);
      own.fromArray(face, t * 3);
      const ownN = own.clone().normalize();
      sum.set(0, 0, 0);
      for (const w of corners) {
        other.fromArray(face, Math.floor(w / 3) * 3);
        if (other.clone().normalize().dot(ownN) > cos) sum.add(other);
      }
      if (sum.lengthSq() < 1e-30) sum.copy(own);
      sum.normalize();
      out[v * 3] = sum.x;
      out[v * 3 + 1] = sum.y;
      out[v * 3 + 2] = sum.z;
    }
  }
  return out;
}
