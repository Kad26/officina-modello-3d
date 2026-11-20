// Shelves: an example of a SketchCraft plugin that asks for sizes (ctx.ask)
// and builds geometry with engine commands. Every board is its own group, so
// Cut list can measure it straight away.

sketchcraft.command("shelves", async (ctx) => {
  const a = await ctx.ask({
    title: "Shelf unit",
    text: "Two sides, a top and bottom, shelves spaced evenly between, and an optional back. Built 1 m to the right of the origin, beside the scale figure.",
    ok: "Build",
    fields: [
      { id: "width", label: "Width", type: "length", value: 0.8, min: 0.2, max: 4 },
      { id: "depth", label: "Depth", type: "length", value: 0.3, min: 0.1, max: 1.2 },
      { id: "height", label: "Height", type: "length", value: 1.8, min: 0.3, max: 3 },
      { id: "shelves", label: "Shelves between", type: "number", value: 3, min: 0, max: 12 },
      { id: "board", label: "Board thickness", type: "length", value: 0.018, min: 0.006, max: 0.05 },
      { id: "back", label: "Add a 6 mm back panel", type: "check", value: true },
    ],
  });
  if (!a) return; // cancelled

  const { width: W, depth: D, height: H, board: T } = a;
  const n = Math.round(a.shelves);
  if (W <= 2 * T) return ctx.toast("The unit is too narrow for its boards.");

  // a board: a box from corner a to corner b, as its own group (1 m along red)
  const X = 1;
  const board = async (name, [x0, y0, z0], [x1, y1, z1]) => {
    x0 += X;
    x1 += X;
    await ctx.run({ cmd: "begin_group", name });
    const f = await ctx.run({ cmd: "polygon", points: [[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0]] });
    if (f.ok) await ctx.run({ cmd: "pushpull", face: f.face, distance: z1 - z0 });
    await ctx.run({ cmd: "end_group" });
  };

  await board("Side", [0, 0, 0], [T, D, H]);
  await board("Side", [W - T, 0, 0], [W, D, H]);
  await board("Bottom", [T, 0, 0], [W - T, D, T]);
  await board("Top", [T, 0, H - T], [W - T, D, H]);
  const gap = (H - 2 * T - n * T) / (n + 1);
  for (let i = 1; i <= n; i++) {
    const z = T + i * gap + (i - 1) * T;
    await board("Shelf", [T, 0, z], [W - T, D, z + T]);
  }
  if (a.back) await board("Back", [0, D, 0], [W, D + 0.006, H]);
  ctx.toast(`Built a shelf unit: ${4 + n + (a.back ? 1 : 0)} boards. File › Plugins › Cut list… lists them.`);
});
