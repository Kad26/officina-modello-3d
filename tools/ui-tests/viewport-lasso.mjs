// Run against the dev page and a bridge with the same scratch SKETCHCRAFT_DOC.
import assert from "node:assert/strict";
import { launch, PAGE, pageExec } from "../demos/lib.mjs";

if (!process.env.SKETCHCRAFT_DOC) throw new Error("Set SKETCHCRAFT_DOC to a scratch model path.");

const browser = await launch();
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  await page.addInitScript(() => localStorage.setItem("sketchcraft.layout", "side"));
  await page.goto(PAGE);
  const run = async (command) => {
    const reply = await pageExec(page, command);
    assert(reply.ok, reply.error);
    return reply;
  };
  await run({ cmd: "new" });
  const { face } = await run({ cmd: "rect", from: [0, 0, 0], to: [4, 4, 0] });
  await run({ cmd: "group", ids: [face], name: "Lasso sample" });
  await page.reload();
  await page.locator(".viewport canvas").waitFor();
  await page.waitForFunction(() => {
    const target = window.__sketchcraftView?.current?.controls.target;
    return !!target && Math.abs(target.x - 2) < .1 && Math.abs(target.y - 2) < .1;
  });

  const corners = await page.evaluate(() => {
    const view = window.__sketchcraftView.current;
    const r = view.renderer.domElement.getBoundingClientRect();
    return [[0, 0, 0], [4, 0, 0], [4, 4, 0], [0, 4, 0]].map((world) => {
      const p = view.camera.position.clone().set(...world).project(view.camera);
      return { x: r.left + (p.x + 1) * r.width / 2, y: r.top + (1 - p.y) * r.height / 2 };
    });
  });
  const x0 = Math.min(...corners.map((p) => p.x));
  const x1 = Math.max(...corners.map((p) => p.x));
  const y0 = Math.min(...corners.map((p) => p.y));
  const y1 = Math.max(...corners.map((p) => p.y));
  const centerX = (x0 + x1) / 2, centerY = (y0 + y1) / 2;
  const partial = [[centerX - 15, centerY - 15], [centerX + 15, centerY - 15], [centerX + 15, centerY + 15], [centerX - 15, centerY + 15]];
  const full = [[x0 - 15, y0 - 15], [x1 + 15, y0 - 15], [x1 + 15, y1 + 15], [x0 - 15, y1 + 15]];
  const crossingCounterclockwise = process.env.VITE_OFFICINA === "1" && await page.evaluate(() => navigator.platform.startsWith("Win"));
  const orient = (points, crossing) => crossing === crossingCounterclockwise ? [...points].reverse() : points;
  const lasso = page.locator('[data-tool="lasso"][aria-pressed="true"]');
  const sample = page.getByRole("treeitem").filter({ hasText: "Lasso sample" });
  await page.keyboard.press("Shift+Space");
  await lasso.waitFor();

  const trace = async ([start, ...rest], crossing, escape = false) => {
    await page.mouse.move(...start);
    await page.mouse.down();
    for (const point of rest) await page.mouse.move(...point, { steps: 4 });
    const outline = page.locator(".select-lasso");
    await outline.waitFor();
    assert.equal(await outline.evaluate((svg) => svg.classList.contains("crossing")), crossing);
    if (escape) {
      await page.keyboard.press("Escape");
      await outline.waitFor({ state: "detached" });
    }
    await page.mouse.up();
    await outline.waitFor({ state: "detached" });
  };

  await trace(orient(partial, true), true);
  assert.equal(await sample.getAttribute("aria-selected"), "true");
  await trace(orient(partial, false), false);
  assert.equal(await sample.getAttribute("aria-selected"), "false");
  await trace(orient(full, false), false);
  assert.equal(await sample.getAttribute("aria-selected"), "true");
  await trace(orient(partial, false), false, true);
  assert.equal(await sample.getAttribute("aria-selected"), "true", "Escape cancels the stroke without changing selection");

  await page.keyboard.press("Escape");
  const [a, b, c, d] = orient(full, false);
  for (const point of [a, b, c]) await page.mouse.click(...point);
  await page.mouse.dblclick(...d);
  await page.locator(".select-lasso").waitFor({ state: "detached" });
  assert.equal(await sample.getAttribute("aria-selected"), "true", "double click closes and applies the segmented lasso");

  await page.keyboard.press("Space");
  await page.locator('.tool[data-tool="select"][aria-pressed="true"]').waitFor();
  console.log("Lasso shortcut, stroke direction, segmented selection, Escape and Space passed");
} finally {
  await browser.close();
}
