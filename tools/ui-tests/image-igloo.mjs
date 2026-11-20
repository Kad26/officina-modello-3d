import assert from "node:assert/strict";
import { launch, PAGE, pageExec } from "../demos/lib.mjs";

if (!process.env.SKETCHCRAFT_DOC) throw new Error("Set SKETCHCRAFT_DOC to a scratch model path.");

const photo = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jp1sAAAAASUVORK5CYII=";
const browser = await launch();
try {
  const page = await browser.newPage({ viewport: { width: 1240, height: 840 } });
  await page.addInitScript(() => localStorage.setItem("sketchcraft.layout", "side"));
  await page.goto(PAGE);
  const run = async (cmd) => {
    const result = await pageExec(page, cmd);
    assert(result.ok, result.error);
    return result;
  };
  await run({ cmd: "new" });
  await run({ cmd: "rect", from: [0, 0, 0], to: [4, 4, 0] });
  const save = (name, eye, target, image) => run({ cmd: "save_view", name, eye, target, fov: 40,
    up: [0, 0, 1], parallel: false, photo: image });
  const firstEye = [7, -5, 3];
  const oppositeEye = [-4, 7, 3];
  await save("Test photo 1", firstEye, [2, 2, 0], photo);
  await save("Test photo 2", firstEye, [2, 2.4, 0], photo);
  await save("Ordinary scene", [1, 2, 3], [2, 2, 0]);
  await save("Test photo 3", oppositeEye, [2, 2, 0], photo);
  await page.reload();
  const card = page.locator(".igloo-photo");
  const key = (name) => page.keyboard.press(name);
  const selected = (number) => card.filter({ hasText: "Test photo " + number }).waitFor();
  const backdrop = page.locator(".viewport img.backdrop");

  await page.locator("#tray-header-scenes").click();
  await key("i");
  assert.equal(await card.count(), 0);
  await page.getByRole("button", { name: "Test photo 1", exact: true }).click();
  await backdrop.waitFor();
  await key("i");
  await selected(1);
  await page.waitForFunction(() => {
    const { world } = window.__sketchcraftView.current;
    return world.getObjectByName("Matched photo overview")?.children.length === 3;
  });
  await key("Control+ArrowRight");
  await selected(2);
  await key("Shift+ArrowRight");
  await selected(3);
  await key("Control+ArrowRight");
  await selected(3);
  await key("Escape");
  await card.waitFor({ state: "detached" });
  await backdrop.waitFor();
  const eye = () => page.evaluate(() => window.__sketchcraftView.current.camera.position.toArray());
  const near = (a, b) => a.every((n, i) => Math.abs(n - b[i]) < 1e-3);
  assert(near(await eye(), firstEye), "Escape restores the matched camera");

  await key("i");
  await selected(1);
  await page.evaluate(() => {
    const { camera, controls } = window.__sketchcraftView.current;
    camera.position.set(-40, 49, 18);
    controls.update();
  });
  await selected(3);
  await key("Enter");
  await card.waitFor({ state: "detached" });
  await backdrop.waitFor();
  assert(near(await eye(), oppositeEye), "Enter restores the selected matched photo scene camera");
  console.log("Image Igloo: orbit, arrow orders, Escape, photo selection and scene camera passed");
} finally {
  await browser.close();
}
