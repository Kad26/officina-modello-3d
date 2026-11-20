import assert from "node:assert/strict";
import test from "node:test";
import { Matrix4, PerspectiveCamera } from "three";
import { lassoDirection, selectLasso, selectMarquee } from "../src/viewport/marquee.ts";

const rect = { left: -0.2, right: 0.2, bottom: -0.2, top: 0.2 };
const ortho = new Matrix4();
const scene = (geometry) => ({
  groups: [], instances: [], faces: [], edgeList: [], edges: [], edgeIds: [],
  positions: [], triFace: [], editPositions: [], editTriFace: [], editing: null, activeGroup: null, tags: {},
  ...geometry,
});

test("crossing picks a long wire profile without either endpoint in the rectangle", () => {
  const a = [-0.9, 0, 0], b = [0.9, 0, 0];
  const model = scene({
    edges: [...a, ...b],
    edgeIds: [5],
    edgeList: [{ id: 5, a, b, soft: false, group: null }],
  });
  assert.deepEqual(selectMarquee(model, ortho, rect, true).edges, [5]);
  assert.deepEqual(selectMarquee(model, ortho, rect, false).edges, []);
});

test("crossing picks a covering face but misses a window cut out of it", () => {
  const hugeFace = scene({
    faces: [{ id: 1, hidden: false, group: null }],
    positions: [-0.9, -0.9, 0, 0.9, -0.9, 0, 0, 0.9, 0],
    triFace: [1],
  });
  assert.deepEqual(selectMarquee(hugeFace, ortho, rect, true).faces, [1]);
  assert.deepEqual(selectMarquee(hugeFace, ortho, rect, false).faces, []);
  const positions = [];
  const strip = (x0, y0, x1, y1) => positions.push(x0, y0, 0, x1, y0, 0, x1, y1, 0, x0, y0, 0, x1, y1, 0, x0, y1, 0);
  strip(-0.9, -0.9, -0.3, 0.9);
  strip(0.3, -0.9, 0.9, 0.9);
  strip(-0.3, -0.9, 0.3, -0.3);
  strip(-0.3, 0.3, 0.3, 0.9);
  const hole = scene({ faces: hugeFace.faces, positions, triFace: Array(8).fill(1) });
  assert.deepEqual(selectMarquee(hole, ortho, rect, true).faces, []);
});

test("wire-only placement follows the editable group boundary", () => {
  const model = scene({
    groups: [{ id: 10, parent: null }, { id: 11, parent: 10 }],
    instances: [{ id: 20, parent: 11 }],
    edges: [-0.9, 0, 0, 0.9, 0, 0],
    edgeIds: [20],
  });
  assert.deepEqual(selectMarquee(model, ortho, rect, true), { groups: [10], faces: [], edges: [], instances: [] });
  assert.deepEqual(selectMarquee({ ...model, activeGroup: 10 }, ortho, rect, true).groups, [11]);
  assert.deepEqual(selectMarquee({ ...model, activeGroup: 11 }, ortho, rect, true).instances, [20]);
  assert.deepEqual(selectMarquee(model, ortho, rect, false).groups, []);
  const wholeView = { left: -1, right: 1, bottom: -1, top: 1 };
  assert.deepEqual(selectMarquee(model, ortho, wholeView, false).groups, [10]);
});

test("fully-contained face selection considers every triangle vertex", () => {
  const model = scene({
    faces: [{ id: 1, hidden: false, group: null }],
    positions: [0, 0, 0, 0.9, -0.9, 0, 0.9, 0.9, 0],
    triFace: [1],
  });
  assert.deepEqual(selectMarquee(model, ortho, rect, true).faces, [1]);
  assert.deepEqual(selectMarquee(model, ortho, rect, false).faces, []);
});

test("perspective clipping excludes lines behind the eye and keeps a crossing at the near plane", () => {
  const camera = new PerspectiveCamera(60, 1, 0.1, 10);
  const model = scene({
    edges: [-0.1, 0, 1, 0.1, 0, 1, 0, 0, 1, 0, 0, -1],
    edgeIds: [5, 6],
    edgeList: [{ id: 5, soft: false, group: null }, { id: 6, soft: false, group: null }],
  });
  assert.deepEqual(selectMarquee(model, camera.projectionMatrix, rect, true).edges, [6]);
  assert.deepEqual(selectMarquee(model, camera.projectionMatrix, rect, false).edges, []);
});

const path = (points) => points.map(([x, y]) => ({ x, y }));

test("the desktop lasso distinguishes clockwise from counterclockwise strokes", () => {
  const clockwise = path([[10, 10], [100, 10], [100, 80], [10, 80], [10, 10]]);
  assert.equal(lassoDirection(clockwise), 1);
  assert.equal(lassoDirection([...clockwise].reverse()), -1);
  assert.equal(lassoDirection(path([[0, 0], [10, 10], [20, 20]])), 0);
});

test("a concave lasso misses its notch, and a contained edge cannot bridge the notch", () => {
  const notch = path([[-.8, -.8], [.8, -.8], [.8, .8], [.2, .8], [.2, 0], [-.2, 0], [-.2, .8], [-.8, .8]]);
  const model = scene({
    edges: [-.1, .5, 0, .1, .5, 0, -.5, .5, 0, .5, .5, 0, -.7, .1, 0, -.6, .4, 0],
    edgeIds: [5, 6, 7],
    edgeList: [5, 6, 7].map((id) => ({ id, group: null })),
  });
  assert.deepEqual(selectLasso(model, ortho, notch, true).edges, [6, 7]);
  assert.deepEqual(selectLasso(model, ortho, notch, false).edges, [7]);
});

test("a lasso on a face selects its owner and clips geometry behind a perspective camera", () => {
  const loop = path([[-.2, -.2], [.2, -.2], [.2, .2], [-.2, .2]]);
  const camera = new PerspectiveCamera(60, 1, .1, 10);
  const model = scene({
    positions: [-1, -1, -1, 1, -1, -1, 0, 1, -1, -1, -1, 1, 1, -1, 1, 0, 1, 1],
    triFace: [11, 12],
    faces: [{ id: 11, group: 10 }, { id: 12, group: null }],
    groups: [{ id: 10, parent: null }],
  });
  assert.deepEqual(selectLasso(model, camera.projectionMatrix, loop, true), { faces: [], edges: [], groups: [10], instances: [] });
  assert.deepEqual(selectLasso({ ...model, activeGroup: 10 }, camera.projectionMatrix, loop, true).faces, [11]);
  assert.deepEqual(selectLasso(model, camera.projectionMatrix, loop, false).groups, []);
});
