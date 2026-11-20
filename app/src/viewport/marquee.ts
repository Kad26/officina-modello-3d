import { Vector4, type Matrix4 } from "three";
import type { Scene } from "../engine";
import type { Sel } from "../selection";

/** The selection rectangle in normalized device coordinates. */
export interface MarqueeRect {
  left: number;
  right: number;
  bottom: number;
  top: number;
}

type Plane = (point: Vector4) => number;
type Target = { kind: keyof Sel; id: number };
const depthPlanes: Plane[] = [({ z, w }) => z + w, ({ z, w }) => w - z];

function clipCrosses(points: Vector4[], planes: Plane[]): boolean {
  // Clip against the near/far planes as well as the four sides of the
  // marquee, before dividing by w. Points behind the eye cannot wrap across it.
  if (points.length === 2) {
    let enter = 0;
    let exit = 1;
    for (const plane of planes) {
      const a = plane(points[0]), b = plane(points[1]);
      if (a < 0 && b < 0) return false;
      if (a < 0) enter = Math.max(enter, a / (a - b));
      if (b < 0) exit = Math.min(exit, a / (a - b));
      if (enter > exit) return false;
    }
    return true;
  }
  return clipPolygon(points, planes).length > 0;
}

function clipPolygon(points: Vector4[], planes: Plane[]): Vector4[] {
  for (const plane of planes) {
    const clipped: Vector4[] = [];
    let a = points[points.length - 1];
    let da = plane(a);
    for (const b of points) {
      const db = plane(b);
      if ((da < 0) !== (db < 0)) clipped.push(a.clone().lerp(b, da / (da - db)));
      if (db >= 0) clipped.push(b);
      a = b;
      da = db;
    }
    if (!clipped.length) return clipped;
    points = clipped;
  }
  return points;
}

/** Select primitives from the same visible triangles and world edges the viewport draws. */
export function selectMarquee(scene: Scene, viewProjection: Matrix4, rect: MarqueeRect, crossing: boolean): Sel {
  const planes: Plane[] = [
    ({ x, w }) => x - rect.left * w,
    ({ x, w }) => rect.right * w - x,
    ({ y, w }) => y - rect.bottom * w,
    ({ y, w }) => rect.top * w - y,
    ...depthPlanes,
  ];
  const inside = (point: Vector4) => planes.every((plane) => plane(point) >= 0);
  return selectProjected(scene, viewProjection, crossing, (points) =>
    crossing ? points.some(inside) || clipCrosses(points, planes) : points.every(inside),
  );
}

export type LassoPoint = { x: number; y: number };
const epsilon = 1e-9;
const cross = (a: LassoPoint, b: LassoPoint, c: LassoPoint) => (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
const pointAt = (a: LassoPoint, b: LassoPoint, t: number) => ({ x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t });
const onSegment = (a: LassoPoint, b: LassoPoint, p: LassoPoint) =>
  Math.abs(cross(a, b, p)) <= epsilon &&
  p.x >= Math.min(a.x, b.x) - epsilon && p.x <= Math.max(a.x, b.x) + epsilon &&
  p.y >= Math.min(a.y, b.y) - epsilon && p.y <= Math.max(a.y, b.y) + epsilon;

function pathSegments(path: readonly LassoPoint[]): [LassoPoint, LassoPoint][] {
  return path.map((a, i) => [a, path[(i + 1) % path.length]]);
}

/** Screen coordinates go down, so a positive area corresponds to clockwise. */
export function lassoDirection(path: readonly LassoPoint[]): number {
  if (path.length < 3) return 0;
  let area = 0;
  let firstTurn = 0;
  for (let i = 2; i < path.length; i++) {
    const turn = cross(path[0], path[i - 1], path[i]);
    area += turn;
    if (!firstTurn && Math.abs(turn) > epsilon) firstTurn = turn;
  }
  return Math.abs(area) > epsilon ? Math.sign(area) : Math.sign(firstTurn);
}

function membership(p: LassoPoint, sides: readonly [LassoPoint, LassoPoint][]): -1 | 0 | 1 {
  let inside = false;
  for (const [a, b] of sides) {
    if (onSegment(a, b, p)) return 0;
    if ((a.y > p.y) !== (b.y > p.y) && p.x < a.x + (b.x - a.x) * (p.y - a.y) / (b.y - a.y)) inside = !inside;
  }
  return inside ? 1 : -1;
}

/** Parameters where a-b touches or intersects c-d, including collinear overlap. */
function intersections(a: LassoPoint, b: LassoPoint, c: LassoPoint, d: LassoPoint): number[] {
  const dx = b.x - a.x, dy = b.y - a.y;
  const ex = d.x - c.x, ey = d.y - c.y;
  const det = dx * ey - dy * ex;
  if (Math.abs(det) <= epsilon) {
    const lengthSq = dx * dx + dy * dy;
    if (lengthSq <= epsilon ** 2) return onSegment(c, d, a) ? [0] : [];
    if (Math.abs(cross(a, b, c)) > epsilon) return [];
    const tc = ((c.x - a.x) * dx + (c.y - a.y) * dy) / lengthSq;
    const td = ((d.x - a.x) * dx + (d.y - a.y) * dy) / lengthSq;
    const first = Math.max(0, Math.min(tc, td)), last = Math.min(1, Math.max(tc, td));
    return first <= last + epsilon ? [first, last] : [];
  }
  const t = ((c.x - a.x) * ey - (c.y - a.y) * ex) / det;
  const u = ((c.x - a.x) * dy - (c.y - a.y) * dx) / det;
  return t >= -epsilon && t <= 1 + epsilon && u >= -epsilon && u <= 1 + epsilon ? [Math.max(0, Math.min(1, t))] : [];
}

function segmentIntervals(a: LassoPoint, b: LassoPoint, sides: readonly [LassoPoint, LassoPoint][]): LassoPoint[] {
  const hits = [0, 1];
  for (const [c, d] of sides) hits.push(...intersections(a, b, c, d));
  hits.sort((x, y) => x - y);
  const samples: LassoPoint[] = [];
  for (let i = 1; i < hits.length; i++) {
    if (hits[i] - hits[i - 1] > epsilon) samples.push(pointAt(a, b, (hits[i] + hits[i - 1]) / 2));
  }
  return samples;
}

function projectLassoPrimitive(points: Vector4[], crossing: boolean): LassoPoint[] {
  if (!crossing && points.some((p) => depthPlanes.some((plane) => plane(p) < 0))) return [];
  if (crossing && points.length === 2) {
    // A segment needs at most one endpoint clipped per plane.
    points = [...points];
    for (const plane of depthPlanes) {
      const [a, b] = points;
      const da = plane(a), db = plane(b);
      if (da < 0 && db < 0) return [];
      if (da < 0 || db < 0) points[da < 0 ? 0 : 1] = a.clone().lerp(b, da / (da - db));
    }
  } else if (crossing) points = clipPolygon(points, depthPlanes);
  return points.map(({ x, y, w }) => ({ x: x / w, y: y / w }));
}

/** Lasso uses the same ownership and visibility rules as rectangle selection. */
export function selectLasso(scene: Scene, viewProjection: Matrix4, path: readonly LassoPoint[], crossing: boolean): Sel {
  if (!lassoDirection(path)) return { faces: [], edges: [], groups: [], instances: [] };
  const outline = pathSegments(path);
  const minX = Math.min(...path.map((p) => p.x)), maxX = Math.max(...path.map((p) => p.x));
  const minY = Math.min(...path.map((p) => p.y)), maxY = Math.max(...path.map((p) => p.y));
  return selectProjected(scene, viewProjection, crossing, (clipPoints) => {
    const points = projectLassoPrimitive(clipPoints, crossing);
    if (!points.length || points.every((p) => p.x < minX) || points.every((p) => p.x > maxX) ||
      points.every((p) => p.y < minY) || points.every((p) => p.y > maxY)) return false;
    const sides = points.length === 2 ? [[points[0], points[1]] as [LassoPoint, LassoPoint]] : pathSegments(points);
    if (crossing) {
      return points.some((p) => membership(p, outline) >= 0) ||
        sides.some(([a, b]) => outline.some(([c, d]) => intersections(a, b, c, d).length > 0)) ||
        (points.length > 2 && path.some((p) => membership(p, sides) >= 0));
    }
    if (points.some((p) => membership(p, outline) < 0)) return false;
    // A concave notch can cross an edge whose endpoints are both inside.
    if (sides.some(([a, b]) => segmentIntervals(a, b, outline).some((p) => membership(p, outline) < 0))) return false;
    // Even-odd paths can enclose a hole entirely inside a triangle.
    return points.length === 2 || !outline.some(([a, b]) => segmentIntervals(a, b, sides).some((p) => membership(p, sides) > 0));
  });
}

function selectProjected(scene: Scene, viewProjection: Matrix4, crossing: boolean, overlaps: (points: Vector4[]) => boolean): Sel {
  const hits: Record<keyof Sel, Map<number, boolean>> = {
    faces: new Map(), edges: new Map(), groups: new Map(), instances: new Map(),
  };
  const primitive = (target: Target | undefined, xyz: readonly number[], at: number, count: number) => {
    if (!target) return;
    const hit = hits[target.kind];
    if (hit.get(target.id) === crossing) return;
    const points = Array.from({ length: count }, (_, i) => {
      const offset = at + i * 3;
      return new Vector4(xyz[offset], xyz[offset + 1], xyz[offset + 2], 1).applyMatrix4(viewProjection);
    });
    hit.set(target.id, overlaps(points));
  };
  const editing = scene.editing != null;
  const groups = new Map(scene.groups.map((group) => [group.id, group]));
  const groupTargets = new Map<number, Target | undefined>();
  const targetFor = (kind: keyof Sel, id: number, parent: number | null): Target | undefined => {
    if (editing) return parent == null ? { kind, id } : { kind: "groups", id: parent };
    if (parent === scene.activeGroup) return { kind, id };
    if (parent == null) return undefined;
    if (!groupTargets.has(parent)) {
      let child = parent;
      let group = groups.get(child);
      let target: Target | undefined;
      for (let depth = 0; group && depth < groups.size; depth++) {
        if (group.hidden || (group.tag && scene.tags[group.tag] === false)) break;
        if (group.parent === scene.activeGroup) {
          target = { kind: "groups", id: child };
          break;
        }
        if (group.parent == null) break;
        child = group.parent;
        group = groups.get(child);
      }
      groupTargets.set(parent, target);
    }
    return groupTargets.get(parent);
  };
  const faces = new Map(scene.faces.filter((face) => !face.hidden).map((face) => [face.id, targetFor("faces", face.id, face.group)]));
  const edges = new Map(scene.edgeList.filter((edge) => !edge.soft).map((edge) => [edge.id, targetFor("edges", edge.id, edge.group)]));
  const instances = new Map(editing ? [] : scene.instances.map((instance) => [instance.id, targetFor("instances", instance.id, instance.parent)]));
  const positions = editing ? scene.editPositions : scene.positions;
  const triangleOwners = editing ? scene.editTriFace : scene.triFace;
  triangleOwners.forEach((id, i) => primitive(faces.get(id) ?? instances.get(id), positions, i * 9, 3));
  if (editing) {
    for (const edge of scene.edgeList) primitive(edges.get(edge.id), [...edge.a, ...edge.b], 0, 2);
  } else {
    scene.edgeIds.forEach((id, i) => primitive(edges.get(id) ?? instances.get(id), scene.edges, i * 6, 2));
  }
  const ids = (map: Map<number, boolean>) => [...map].filter(([, hit]) => hit).map(([id]) => id);
  return { faces: ids(hits.faces), edges: ids(hits.edges), groups: ids(hits.groups), instances: ids(hits.instances) };
}
