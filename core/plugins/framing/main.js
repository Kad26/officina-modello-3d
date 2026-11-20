// Framing: an example of a plugin tool (sketchcraft.tool). It asks for sizes
// once, then each click pair builds a stud wall: a bottom plate, a double top
// plate and studs at your spacing, every board its own group. Walls chain:
// the next one starts where the last ended. Esc leaves the tool.

let size = null; // { height, spacing, thick, deep }
let start = null; // where the wall being drawn starts

sketchcraft.tool("stud-wall", {
  async start(ctx) {
    size = await ctx.ask({
      title: "Stud wall",
      text: "Sizes for the walls you draw. Every board becomes its own group, so Cut list can count them.",
      ok: "Draw walls",
      fields: [
        { id: "height", label: "Wall height", type: "length", value: 2.4384, min: 1, max: 6 },
        { id: "spacing", label: "Stud spacing (on center)", type: "length", value: 0.4064, min: 0.2, max: 1.2 },
        { id: "thick", label: "Lumber thickness", type: "length", value: 0.0381, min: 0.02, max: 0.1 },
        { id: "deep", label: "Lumber depth", type: "length", value: 0.0889, min: 0.05, max: 0.3 },
      ],
    });
    if (!size) return ctx.finish();
    ctx.hint("Click where the wall starts.");
  },

  move(ctx, at) {
    if (!start || !size) return;
    ctx.preview(wallLines(start, at.point, size));
  },

  async click(ctx, at) {
    if (!size) return;
    if (!start) {
      start = at.point;
      ctx.anchor(start);
      ctx.hint("Click where it ends. Esc when you're done.");
      return;
    }
    const end = at.point;
    if (dist(start, end) < size.thick * 2) return ctx.toast("That wall is too short.");
    await buildWall(ctx, start, end, size);
    // the next wall starts here
    start = end;
    ctx.anchor(start);
    ctx.preview([]);
    ctx.hint("Next wall: click where it ends, or Esc to stop.");
  },
});

const dist = (a, b) => Math.hypot(b[0] - a[0], b[1] - a[1]);

/** Along the wall (u) and across it (v, to the left), on the ground plane. */
function axes(a, b) {
  const L = dist(a, b);
  const u = [(b[0] - a[0]) / L, (b[1] - a[1]) / L];
  return { L, u, v: [-u[1], u[0]] };
}

/** Where the studs go, along the wall: one every `spacing`, and one at the end. */
function studsAt(L, s) {
  const xs = [];
  for (let x = 0; x <= L - s.thick + 1e-9; x += s.spacing) xs.push(x);
  if (L - s.thick - xs[xs.length - 1] > 1e-6) xs.push(L - s.thick);
  return xs;
}

/** Preview: the plates' outline and a line for each stud. */
function wallLines(a, b, s) {
  const { L, u } = axes(a, b);
  if (L < 1e-6) return [];
  const z0 = a[2];
  const at = (x, z) => [a[0] + u[0] * x, a[1] + u[1] * x, z0 + z];
  const lines = [
    [at(0, 0), at(L, 0)],
    [at(0, s.height), at(L, s.height)],
    [at(0, 0), at(0, s.height)],
    [at(L, 0), at(L, s.height)],
  ];
  for (const x of studsAt(L, s)) lines.push([at(x + s.thick / 2, s.thick), at(x + s.thick / 2, s.height - 2 * s.thick)]);
  return lines;
}

async function buildWall(ctx, a, b, s) {
  const { L, u, v } = axes(a, b);
  const z0 = a[2];
  // a board: a footprint (along x0..x1, across 0..deep) pulled up from z to z + h
  const board = async (name, x0, x1, z, h) => {
    const p = (x, y) => [a[0] + u[0] * x + v[0] * y, a[1] + u[1] * x + v[1] * y, z0 + z];
    await ctx.run({ cmd: "begin_group", name });
    const f = await ctx.run({ cmd: "polygon", points: [p(x0, 0), p(x1, 0), p(x1, s.deep), p(x0, s.deep)] });
    if (f.ok) await ctx.run({ cmd: "pushpull", face: f.face, distance: h });
    await ctx.run({ cmd: "end_group" });
  };
  await board("Bottom plate", 0, L, 0, s.thick);
  await board("Top plate", 0, L, s.height - 2 * s.thick, s.thick);
  await board("Top plate", 0, L, s.height - s.thick, s.thick);
  const stud = s.height - 3 * s.thick;
  for (const x of studsAt(L, s)) await board("Stud", x, x + s.thick, s.thick, stud);
}
